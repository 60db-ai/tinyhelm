<?php

use App\Jobs\ApplicationDeploymentJob;
use App\Models\Application;
use App\Models\Environment;
use App\Models\Project;
use App\Models\Server;
use App\Models\StandaloneDocker;
use App\Models\Team;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

uses(TestCase::class, RefreshDatabase::class);

class TestableTinyHelmUrlDeploymentJob extends ApplicationDeploymentJob
{
    public function __construct() {}

    public function execute_remote_command(...$commands): void {}
}

function tinyhelmVariablesForFqdn(string $fqdn, string $composeParsingVersion = '3'): string
{
    $team = Team::create([
        'name' => 'TinyHelm Url Team',
        'personal_team' => false,
        'show_boarding' => false,
    ]);
    $project = Project::create([
        'name' => 'TinyHelm Url Project',
        'team_id' => $team->id,
    ]);
    $environment = Environment::where('project_id', $project->id)->firstOrFail();
    $server = Server::factory()->create(['team_id' => $team->id]);
    $destination = $server->standaloneDockers()->firstOrFail();
    $application = Application::factory()->create([
        'environment_id' => $environment->id,
        'destination_id' => $destination->id,
        'destination_type' => StandaloneDocker::class,
        'build_pack' => 'dockercompose',
        'fqdn' => $fqdn,
    ]);

    // The created hook resets this, so it has to be set afterwards.
    $application->compose_parsing_version = $composeParsingVersion;
    $application->save();
    Application::withoutGlobalScopes()->whereKey($application->id)->update(['fqdn' => $fqdn]);

    $job = new TestableTinyHelmUrlDeploymentJob;
    $reflection = new ReflectionClass(ApplicationDeploymentJob::class);

    foreach ([
        'application' => $application->fresh(),
        'pull_request_id' => 0,
        'commit' => 'HEAD',
    ] as $property => $value) {
        $reflection->getProperty($property)->setValue($job, $value);
    }

    $reflection->getMethod('set_tinyhelm_variables')->invoke($job);

    return $reflection->getProperty('tinyhelm_variables')->getValue($job);
}

it('keeps every domain intact when an application has multiple domains', function () {
    $variables = tinyhelmVariablesForFqdn('https://a.example.com,https://b.example.com');

    expect($variables)
        ->toContain("TINYHELM_URL='https://a.example.com,https://b.example.com'")
        ->toContain("TINYHELM_FQDN='a.example.com,b.example.com'");
});

it('strips the port from every domain when an application has multiple domains', function () {
    $variables = tinyhelmVariablesForFqdn('https://a.example.com:8080,https://b.example.com:9000');

    expect($variables)
        ->toContain("TINYHELM_URL='https://a.example.com,https://b.example.com'")
        ->toContain("TINYHELM_FQDN='a.example.com,b.example.com'");
});

it('keeps every domain intact on the legacy compose parsing version', function () {
    $variables = tinyhelmVariablesForFqdn('https://a.example.com,https://b.example.com', '2');

    expect($variables)
        ->toContain("TINYHELM_URL='a.example.com,b.example.com'")
        ->toContain("TINYHELM_FQDN='https://a.example.com,https://b.example.com'");
});

it('still resolves a single domain', function () {
    $variables = tinyhelmVariablesForFqdn('https://a.example.com');

    expect($variables)
        ->toContain("TINYHELM_URL='https://a.example.com'")
        ->toContain("TINYHELM_FQDN='a.example.com'");
});

it('ignores a hostless stored domain while setting deployment variables', function () {
    $variables = tinyhelmVariablesForFqdn('https://,https://a.example.com');

    expect($variables)
        ->toContain("TINYHELM_URL='https://a.example.com'")
        ->toContain("TINYHELM_FQDN='a.example.com'");
});

it('does not set domain variables for a hostless stored fqdn', function () {
    $variables = tinyhelmVariablesForFqdn('https://');

    expect($variables)
        ->not->toContain('TINYHELM_URL=')
        ->not->toContain('TINYHELM_FQDN=');
});

function tinyhelmEnvFileVariablesForFqdn(string $fqdn, string $composeParsingVersion = '3'): array
{
    tinyhelmVariablesForFqdn($fqdn, $composeParsingVersion);
    $application = Application::withoutGlobalScopes()->latest('id')->firstOrFail();

    $job = new TestableTinyHelmUrlDeploymentJob;
    $reflection = new ReflectionClass(ApplicationDeploymentJob::class);

    foreach ([
        'application' => $application,
        'pull_request_id' => 0,
        'commit' => 'HEAD',
        'branch' => 'main',
    ] as $property => $value) {
        $reflection->getProperty($property)->setValue($job, $value);
    }

    return $reflection->getMethod('generate_tinyhelm_env_variables')->invoke($job, true)->all();
}

it('drops the path from TINYHELM_FQDN in the generated env file', function () {
    $envs = tinyhelmEnvFileVariablesForFqdn('http://x.com/admin');

    expect($envs['TINYHELM_FQDN'])->toBe('x.com')
        ->and($envs['TINYHELM_URL'])->toBe('http://x.com/admin');
});

it('drops the path and port from every domain in the generated env file', function () {
    $envs = tinyhelmEnvFileVariablesForFqdn('https://a.example.com/admin,https://b.example.com:8080/api');

    expect($envs['TINYHELM_FQDN'])->toBe('a.example.com,b.example.com');
});

it('drops the path from TINYHELM_URL in the generated env file on the legacy compose parsing version', function () {
    $envs = tinyhelmEnvFileVariablesForFqdn('http://x.com/admin', '2');

    expect($envs['TINYHELM_URL'])->toBe('x.com')
        ->and($envs['TINYHELM_FQDN'])->toBe('http://x.com/admin');
});
