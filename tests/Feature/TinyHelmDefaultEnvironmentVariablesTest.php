<?php

use App\Actions\Database\StartDragonfly;
use App\Actions\Database\StartPostgresql;
use App\Jobs\ApplicationDeploymentJob;
use App\Livewire\Project\Shared\EnvironmentVariable\All;
use App\Models\Application;
use App\Models\ApplicationDeploymentQueue;
use App\Models\Environment;
use App\Models\InstanceSettings;
use App\Models\Project;
use App\Models\Server;
use App\Models\Service;
use App\Models\ServiceApplication;
use App\Models\StandaloneDragonfly;
use App\Models\StandalonePostgresql;
use App\Models\Team;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Bus;
use Livewire\Livewire;

uses(RefreshDatabase::class);

beforeEach(function () {
    InstanceSettings::query()->firstOrCreate(['id' => 0]);
    Bus::fake();

    $this->team = Team::factory()->create();
    $this->server = Server::factory()->create(['team_id' => $this->team->id]);
    $this->destination = $this->server->standaloneDockers()->firstOrFail();
    $project = Project::factory()->create(['team_id' => $this->team->id, 'name' => 'Shop $live']);
    $this->environment = Environment::factory()->create(['project_id' => $project->id, 'name' => 'staging']);
});

function tinyhelmDefaultsPostgres(array $attributes = []): StandalonePostgresql
{
    return StandalonePostgresql::forceCreate([
        'uuid' => 'tinyhelm-defaults-db',
        'name' => 'Main database',
        'image' => 'postgres:16-alpine',
        'postgres_password' => 'password',
        'environment_id' => test()->environment->id,
        'destination_id' => test()->destination->id,
        'destination_type' => test()->destination->getMorphClass(),
        ...$attributes,
    ]);
}

/**
 * @return array<int, string>
 */
function tinyhelmDefaultsDatabaseEnvironment(object $action, $database): array
{
    $action->database = $database->fresh();

    return (new ReflectionMethod($action, 'generate_environment_variables'))->invoke($action);
}

/**
 * @return array<int, string>
 */
function tinyhelmDefaultsApplicationEnv(Application $application, bool $forBuildTime = false, ?Server $deploymentServer = null): array
{
    $reflection = new ReflectionClass(ApplicationDeploymentJob::class);
    $job = $reflection->newInstanceWithoutConstructor();
    $queue = Mockery::mock(ApplicationDeploymentQueue::class);
    $queue->shouldReceive('addLogEntry')->andReturnNull();

    $properties = [
        'application' => $application->fresh(),
        'application_deployment_queue' => $queue,
        'build_pack' => 'nixpacks',
        'mainServer' => $deploymentServer ?? $application->destination->server,
        'pull_request_id' => 0,
        'commit' => 'HEAD',
        'branch' => 'main',
        'container_name' => 'tinyhelm-defaults-app',
        'saved_outputs' => collect(),
    ];
    foreach ($properties as $property => $value) {
        $reflection->getProperty($property)->setValue($job, $value);
    }

    if ($forBuildTime) {
        return collect($reflection->getMethod('generate_tinyhelm_env_variables')->invoke($job, true))
            ->map(fn ($value, $key) => "{$key}={$value}")->values()->all();
    }

    return collect($reflection->getMethod('generate_runtime_environment_variables')->invoke($job))->all();
}

function tinyhelmDefaultsApplication(): Application
{
    return Application::factory()->create([
        'name' => 'My web app',
        'environment_id' => test()->environment->id,
        'destination_id' => test()->destination->id,
        'destination_type' => test()->destination->getMorphClass(),
    ]);
}

function tinyhelmDefaultsService(string $environmentYaml = ''): Service
{
    $service = Service::factory()->create([
        'name' => 'blog',
        'environment_id' => test()->environment->id,
        'server_id' => test()->server->id,
        'destination_id' => test()->destination->id,
        'destination_type' => test()->destination->getMorphClass(),
        'docker_compose_raw' => "services:\n  app:\n    image: nginx:latest\n{$environmentYaml}",
    ]);
    ServiceApplication::create(['name' => 'app', 'service_id' => $service->id]);

    return $service;
}

test('a database gets the same TinyHelm variables as other resources, with compose-safe values', function () {
    $environment = tinyhelmDefaultsDatabaseEnvironment(new StartPostgresql, tinyhelmDefaultsPostgres());

    expect($environment)
        ->toContain('TINYHELM_RESOURCE_UUID=tinyhelm-defaults-db')
        ->toContain('TINYHELM_CONTAINER_NAME=tinyhelm-defaults-db')
        ->toContain('TINYHELM_APP_NAME=Main database')
        ->toContain('TINYHELM_PROJECT_NAME=Shop $$live')
        ->toContain('TINYHELM_ENVIRONMENT_NAME=staging')
        ->toContain("TINYHELM_SERVER_IP={$this->server->ip}");
});

test('a Dragonfly database also gets the TinyHelm variables', function () {
    $database = StandaloneDragonfly::forceCreate([
        'uuid' => 'tinyhelm-defaults-dragonfly',
        'name' => 'Cache',
        'image' => 'docker.dragonflydb.io/dragonflydb/dragonfly',
        'dragonfly_password' => 'password',
        'environment_id' => $this->environment->id,
        'destination_id' => $this->destination->id,
        'destination_type' => $this->destination->getMorphClass(),
    ]);

    expect(tinyhelmDefaultsDatabaseEnvironment(new StartDragonfly, $database))
        ->toContain('TINYHELM_RESOURCE_UUID=tinyhelm-defaults-dragonfly')
        ->toContain('TINYHELM_APP_NAME=Cache');
});

test('a database variable that the user defines wins over the TinyHelm variable', function () {
    $database = tinyhelmDefaultsPostgres();
    $database->runtime_environment_variables()->create(['key' => 'TINYHELM_APP_NAME', 'value' => 'custom']);

    $appNames = collect(tinyhelmDefaultsDatabaseEnvironment(new StartPostgresql, $database))
        ->filter(fn ($variable) => str_starts_with($variable, 'TINYHELM_APP_NAME='));

    expect($appNames->values()->all())->toBe(['TINYHELM_APP_NAME=custom']);
});

test('a database without the option gets no default variables but keeps its identity variables', function () {
    $environment = tinyhelmDefaultsDatabaseEnvironment(new StartPostgresql, tinyhelmDefaultsPostgres(['include_tinyhelm_default_variables' => false]));

    expect(collect($environment)->first(fn ($variable) => str_starts_with($variable, 'TINYHELM_APP_NAME=')))->toBeNull()
        ->and(collect($environment)->first(fn ($variable) => str_starts_with($variable, 'TINYHELM_SERVER_IP=')))->toBeNull()
        ->and($environment)->toContain('TINYHELM_RESOURCE_UUID=tinyhelm-defaults-db');
});

test('an application gets the default variables as quoted lines in its runtime .env file', function () {
    $environment = tinyhelmDefaultsApplicationEnv(tinyhelmDefaultsApplication());

    expect($environment)
        ->toContain('TINYHELM_APP_NAME="My web app"')
        ->toContain('TINYHELM_PROJECT_NAME="Shop \\$live"')
        ->toContain('TINYHELM_ENVIRONMENT_NAME="staging"')
        ->toContain("TINYHELM_SERVER_IP=\"{$this->server->ip}\"");
});

test('an application on an additional server gets the IP of the server it runs on', function () {
    $additionalServer = Server::factory()->create(['team_id' => $this->team->id, 'ip' => '203.0.113.7']);

    expect(tinyhelmDefaultsApplicationEnv(tinyhelmDefaultsApplication(), deploymentServer: $additionalServer))
        ->toContain('TINYHELM_SERVER_IP="203.0.113.7"');
});

test('an application build does not get the default variables', function () {
    $environment = tinyhelmDefaultsApplicationEnv(tinyhelmDefaultsApplication(), forBuildTime: true);

    expect(collect($environment)->first(fn ($variable) => str_starts_with($variable, 'TINYHELM_APP_NAME=')))->toBeNull()
        ->and(collect($environment)->first(fn ($variable) => str_starts_with($variable, 'TINYHELM_RESOURCE_UUID=')))->not->toBeNull();
});

test('an application without the option gets no default variables', function () {
    $application = tinyhelmDefaultsApplication();
    $application->settings->update(['include_tinyhelm_default_variables' => false]);

    $environment = tinyhelmDefaultsApplicationEnv($application);

    expect(collect($environment)->first(fn ($variable) => str_starts_with($variable, 'TINYHELM_APP_NAME=')))->toBeNull();
});

test('a service variable defined in the compose file wins over the TinyHelm variable', function () {
    $service = tinyhelmDefaultsService("    environment:\n      TINYHELM_APP_NAME: custom\n");

    $environment = data_get(serviceParser($service), 'services.app.environment');

    expect($environment['TINYHELM_APP_NAME'])->toBe('custom')
        ->and($environment['TINYHELM_PROJECT_NAME'])->toBe('Shop $$live');
});

test('a service without the option gets no default variables', function () {
    $service = tinyhelmDefaultsService();
    $service->forceFill(['include_tinyhelm_default_variables' => false])->save();

    $environment = collect(data_get(serviceParser($service->fresh()), 'services.app.environment'));

    expect($environment->has('TINYHELM_APP_NAME'))->toBeFalse()
        ->and($environment->has('TINYHELM_RESOURCE_UUID'))->toBeTrue();
});

test('a team admin turns the option off from the environment variables page', function (string $resourceType) {
    $user = User::factory()->create();
    $this->team->members()->attach($user, ['role' => 'admin']);
    $this->actingAs($user);
    $resource = match ($resourceType) {
        'application' => tinyhelmDefaultsApplication(),
        'service' => tinyhelmDefaultsService(),
        'database' => tinyhelmDefaultsPostgres(),
    };

    Livewire::test(All::class, ['resource' => $resource])
        ->assertSet('include_tinyhelm_default_variables', true)
        ->set('include_tinyhelm_default_variables', false)
        ->call('saveTinyHelmDefaultVariables');

    $owner = $resource instanceof Application ? $resource->settings->fresh() : $resource->fresh();
    expect($owner->include_tinyhelm_default_variables)->toBeFalse();
})->with(['application', 'service', 'database']);

test('a team member cannot change the option', function () {
    $user = User::factory()->create();
    $this->team->members()->attach($user, ['role' => 'member']);
    $this->actingAs($user);
    $database = tinyhelmDefaultsPostgres();

    Livewire::test(All::class, ['resource' => $database])
        ->set('include_tinyhelm_default_variables', false)
        ->call('saveTinyHelmDefaultVariables');

    expect($database->fresh()->include_tinyhelm_default_variables)->toBeTrue();
});

test('a compose application gets its resource UUID in the .env file, so that the compose file can reference it', function () {
    $application = tinyhelmDefaultsApplication();
    $application->update(['build_pack' => 'dockercompose']);

    expect((int) $application->fresh()->compose_parsing_version)->toBeGreaterThanOrEqual(3)
        ->and(tinyhelmDefaultsApplicationEnv($application))->toContain("TINYHELM_RESOURCE_UUID=\"{$application->uuid}\"");
});
