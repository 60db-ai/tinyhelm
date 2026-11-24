<?php

use App\Actions\Server\UpdateTinyHelm;
use App\Livewire\Settings\Updates;
use App\Models\InstanceSettings;
use App\Models\PrivateKey;
use App\Models\ScheduledDatabaseBackup;
use App\Models\Server;
use App\Models\StandaloneDocker;
use App\Models\StandalonePostgresql;
use App\Models\Team;
use App\Models\User;
use App\Support\RemoteProcessCommand;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Notification;
use Illuminate\Support\Facades\Process;
use Illuminate\Support\Facades\Queue;
use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Once;
use Livewire\Livewire;
use Spatie\Activitylog\Models\Activity;
use Tests\TestCase;

uses(TestCase::class, RefreshDatabase::class);

function updateTinyHelmTestCreateRootServerAndSettings(array $settings = []): void
{
    Team::factory()->create(['id' => 0]);
    Server::forceCreate([
        'id' => 0,
        'name' => 'localhost',
        'ip' => '127.0.0.1',
        'user' => 'root',
        'team_id' => 0,
        'private_key_id' => 1,
    ]);
    InstanceSettings::forceCreate(array_merge([
        'id' => 0,
        'is_auto_update_enabled' => true,
        'auto_update_frequency' => '0 0 * * *',
        'update_check_frequency' => '0 * * * *',
    ], $settings));
    Once::flush();
}

function updateTinyHelmTestActionWithStatus(string $status): UpdateTinyHelm
{
    return new class($status) extends UpdateTinyHelm
    {
        public function __construct(private string $status) {}

        protected function readUpgradeStatus(): ?string
        {
            return $this->status;
        }
    };
}

function updateTinyHelmTestCreateInstanceBackup(): void
{
    Storage::fake('ssh-keys');
    Notification::fake();
    Server::query()->whereKey(0)->update(['private_key_id' => PrivateKey::factory()->create(['team_id' => 0])->id]);
    Server::flushIdentityMap();
    StandalonePostgresql::forceCreate([
        'id' => 0,
        'name' => 'tinyhelm-db',
        'postgres_password' => 'password',
        'destination_type' => StandaloneDocker::class,
        'destination_id' => 0,
    ]);
    ScheduledDatabaseBackup::create([
        'frequency' => '0 0 * * *',
        'database_id' => 0,
        'database_type' => StandalonePostgresql::class,
        'team_id' => 0,
    ]);
}

function updateTinyHelmTestPrepareUpgrade(): void
{
    Queue::fake();
    config([
        'app.env' => 'testing',
        'constants.tinyhelm.version' => '4.0.9',
        'constants.tinyhelm.helper_version' => '1.0.14',
        'constants.tinyhelm.upgrade_script_url' => 'https://cdn.example.com/upgrade.sh',
        'constants.ssh.mux_enabled' => false,
    ]);

    updateTinyHelmTestCreateRootServerAndSettings([
        'is_auto_update_enabled' => true,
        'new_version_available' => true,
        'docker_registry_url' => 'docker.io',
    ]);

    Http::fake([
        '*' => Http::response([
            'stable' => ['tinyhelm' => ['version' => '4.0.10']],
        ], 200),
    ]);
}

function updateTinyHelmTestPrepareUpdate(array $settings = []): void
{
    updateTinyHelmTestPrepareUpgrade();
    InstanceSettings::findOrFail(0)->update($settings);
    updateTinyHelmTestCreateInstanceBackup();
}

afterEach(function () {
    Mockery::close();
});

it('validates cache against running version before fallback', function () {
    updateTinyHelmTestCreateRootServerAndSettings();

    // CDN fails
    Http::fake(['*' => Http::response(null, 500)]);

    // Mock cache returning older version
    Cache::shouldReceive('remember')
        ->andReturn(['tinyhelm' => ['version' => '4.0.5']]);

    config(['constants.tinyhelm.version' => '4.0.10']);

    $action = new UpdateTinyHelm;

    // Should throw exception - cache is older than running
    try {
        $action->handle(manual_update: false);
        expect(false)->toBeTrue('Expected exception was not thrown');
    } catch (Exception $e) {
        expect($e->getMessage())->toContain('cache version');
        expect($e->getMessage())->toContain('4.0.5');
        expect($e->getMessage())->toContain('4.0.10');
    }
});

it('uses validated cache when CDN fails and cache is newer', function () {
    updateTinyHelmTestCreateRootServerAndSettings();
    Queue::fake();
    config(['constants.ssh.mux_enabled' => false]);

    // CDN fails
    Http::fake(['*' => Http::response(null, 500)]);

    // Cache has newer version than current
    Cache::shouldReceive('remember')
        ->andReturn(['tinyhelm' => ['version' => '4.0.10']]);

    config(['constants.tinyhelm.version' => '4.0.5']);

    $action = new UpdateTinyHelm;

    Log::shouldReceive('warning')
        ->once()
        ->with('Failed to fetch fresh version from CDN, using validated cache', Mockery::type('array'));

    // Should not throw - cache (4.0.10) > running (4.0.5)
    $action->handle(manual_update: false);

    expect($action->latestVersion)->toBe('4.0.10');
});

it('passes the saved registry URL to the upgrade script command', function () {
    Queue::fake();
    config([
        'app.env' => 'testing',
        'constants.tinyhelm.version' => '4.0.9',
        'constants.tinyhelm.helper_version' => '1.0.14',
        'constants.tinyhelm.upgrade_script_url' => 'https://cdn.example.com/upgrade.sh',
        'constants.ssh.mux_enabled' => false,
    ]);

    updateTinyHelmTestCreateRootServerAndSettings([
        'is_auto_update_enabled' => true,
        'docker_registry_url' => 'ghcr.io',
    ]);

    Http::fake([
        '*' => Http::response([
            'stable' => ['tinyhelm' => ['version' => '4.0.10']],
        ], 200),
    ]);

    (new UpdateTinyHelm)->handle();

    expect(RemoteProcessCommand::read(Activity::query()->latest('id')->first()))->toBe(
        "curl -fsSL https://cdn.example.com/upgrade.sh -o /data/tinyhelm/source/upgrade.sh\n".
        "bash /data/tinyhelm/source/upgrade.sh '4.0.10' '1.0.14' 'ghcr.io'"
    );
});

it('falls back to docker io for the upgrade script command when no registry is saved', function () {
    Queue::fake();
    config([
        'app.env' => 'testing',
        'constants.tinyhelm.version' => '4.0.9',
        'constants.tinyhelm.helper_version' => '1.0.14',
        'constants.tinyhelm.registry_url' => 'ghcr.io',
        'constants.tinyhelm.upgrade_script_url' => 'https://cdn.example.com/upgrade.sh',
        'constants.ssh.mux_enabled' => false,
    ]);

    updateTinyHelmTestCreateRootServerAndSettings([
        'is_auto_update_enabled' => true,
    ]);

    Http::fake([
        '*' => Http::response([
            'stable' => ['tinyhelm' => ['version' => '4.0.10']],
        ], 200),
    ]);

    (new UpdateTinyHelm)->handle();

    expect(RemoteProcessCommand::read(Activity::query()->latest('id')->first()))->toBe(
        "curl -fsSL https://cdn.example.com/upgrade.sh -o /data/tinyhelm/source/upgrade.sh\n".
        "bash /data/tinyhelm/source/upgrade.sh '4.0.10' '1.0.14' 'docker.io'"
    );
});

it('defaults the registry setting to docker io when no registry is saved', function () {
    config([
        'app.env' => 'testing',
        'constants.tinyhelm.registry_url' => 'ghcr.io',
        'constants.tinyhelm.self_hosted' => true,
    ]);

    updateTinyHelmTestCreateRootServerAndSettings();

    $rootTeam = Team::findOrFail(0);
    $user = User::factory()->create();
    $rootTeam->members()->attach($user->id, ['role' => 'admin']);

    $this->actingAs($user);
    session(['currentTeam' => ['id' => $rootTeam->id]]);

    Livewire::test(Updates::class)
        ->assertSet('docker_registry_url', 'docker.io');
});

it('uses the database registry for helper images when the configured helper image is default', function () {
    config([
        'constants.tinyhelm.registry_url' => 'ghcr.io',
        'constants.tinyhelm.helper_image' => 'ghcr.io/coollabsio/tinyhelm-helper',
    ]);

    updateTinyHelmTestCreateRootServerAndSettings([
        'docker_registry_url' => 'docker.io',
    ]);

    expect(tinyhelmRegistryUrl())->toBe('docker.io')
        ->and(tinyhelmHelperImage())->toBe('docker.io/coollabsio/tinyhelm-helper');
});

it('preserves an explicit custom helper image override', function () {
    config([
        'constants.tinyhelm.registry_url' => 'docker.io',
        'constants.tinyhelm.helper_image' => 'registry.example.com/custom/helper',
    ]);

    updateTinyHelmTestCreateRootServerAndSettings([
        'docker_registry_url' => 'ghcr.io',
    ]);

    expect(tinyhelmHelperImage())->toBe('registry.example.com/custom/helper');
});

it('rejects invalid registry values and does not sync them', function () {
    Process::fake();
    config([
        'app.env' => 'testing',
        'constants.tinyhelm.registry_url' => 'docker.io',
    ]);

    updateTinyHelmTestCreateRootServerAndSettings([
        'is_auto_update_enabled' => true,
        'auto_update_frequency' => '0 0 * * *',
        'update_check_frequency' => '0 * * * *',
        'docker_registry_url' => 'docker.io',
    ]);

    $rootTeam = Team::findOrFail(0);
    $user = User::factory()->create();
    $rootTeam->members()->attach($user->id, ['role' => 'admin']);

    $this->actingAs($user);
    session(['currentTeam' => ['id' => $rootTeam->id]]);

    Livewire::test(Updates::class)
        ->set('docker_registry_url', 'ghcr.io; touch /tmp/pwned')
        ->call('submit')
        ->assertHasErrors(['docker_registry_url' => ['in']]);

    expect(InstanceSettings::findOrFail(0)->docker_registry_url)->toBe('docker.io');
    Process::assertDidntRun(fn () => true);
});

it('does not save registry changes when syncing the env file fails', function () {
    config([
        'app.env' => 'testing',
        'constants.tinyhelm.registry_url' => 'docker.io',
        'constants.tinyhelm.self_hosted' => true,
    ]);

    updateTinyHelmTestCreateRootServerAndSettings([
        'is_auto_update_enabled' => true,
        'auto_update_frequency' => '0 0 * * *',
        'update_check_frequency' => '0 * * * *',
        'docker_registry_url' => 'docker.io',
    ]);

    $rootTeam = Team::findOrFail(0);
    $user = User::factory()->create();
    $rootTeam->members()->attach($user->id, ['role' => 'admin']);

    $this->actingAs($user);
    session(['currentTeam' => ['id' => $rootTeam->id]]);

    $component = new class extends Updates
    {
        protected function syncRegistryUrlToEnv(string $registryUrl): void
        {
            throw new RuntimeException('sync failed');
        }
    };
    $component->settings = InstanceSettings::findOrFail(0);
    $component->auto_update_frequency = '0 0 * * *';
    $component->update_check_frequency = '0 * * * *';
    $component->is_auto_update_enabled = true;
    $component->docker_registry_url = 'ghcr.io';

    $component->instantSave();

    expect(InstanceSettings::findOrFail(0)->docker_registry_url)->toBe('docker.io');
});

it('appends registry url to env file when the key is missing', function () {
    $component = new Updates;
    $method = new ReflectionMethod(Updates::class, 'registryEnvSyncCommand');

    $server = new Server;
    $server->user = 'cooluser';
    $lines = parseCommandsByLineForSudo(collect($method->invoke($component, 'ghcr.io')), $server);

    // Each branch runs through sudo for a non-root SSH user; nothing is opened by the SSH user's shell.
    expect($lines)->toBe([
        "if sudo grep -q '^REGISTRY_URL=' /data/tinyhelm/source/.env; then",
        "sudo     sed -i 's|^REGISTRY_URL=.*|REGISTRY_URL=ghcr.io|' /data/tinyhelm/source/.env",
        'else',
        "sudo     printf '%s\\n' 'REGISTRY_URL=ghcr.io' | sudo tee -a /data/tinyhelm/source/.env > /dev/null",
        'fi',
    ]);
});

it('prevents downgrade even with manual update', function () {
    updateTinyHelmTestCreateRootServerAndSettings();

    // CDN returns older version
    Http::fake([
        '*' => Http::response([
            'stable' => ['tinyhelm' => ['version' => '4.0.0']],
        ], 200),
    ]);

    // Current version is newer
    config(['constants.tinyhelm.version' => '4.0.10']);

    $action = new UpdateTinyHelm;

    Log::shouldReceive('error')
        ->once()
        ->with('Downgrade prevented', Mockery::type('array'));

    // Should throw exception even for manual updates
    try {
        $action->handle(manual_update: true);
        expect(false)->toBeTrue('Expected exception was not thrown');
    } catch (Exception $e) {
        expect($e->getMessage())->toContain('Cannot downgrade');
        expect($e->getMessage())->toContain('4.0.10');
        expect($e->getMessage())->toContain('4.0.0');
    }
});

it('does not start a second upgrade while a recent upgrade status is in progress', function () {
    Queue::fake();
    config([
        'app.env' => 'testing',
        'constants.tinyhelm.version' => '4.0.9',
        'constants.ssh.mux_enabled' => false,
    ]);
    updateTinyHelmTestCreateRootServerAndSettings();
    Http::fake(['*' => Http::response(['stable' => ['tinyhelm' => ['version' => '4.0.10']]], 200)]);
    $action = updateTinyHelmTestActionWithStatus('3|Pulling Docker images|'.now()->subMinutes(2)->toIso8601String());

    expect(fn () => $action->handle(manual_update: true))
        ->toThrow(Exception::class, 'Another TinyHelm upgrade is already running');

    expect(Activity::query()->count())->toBe(0)
        ->and(InstanceSettings::findOrFail(0)->new_version_available)->toBeFalsy();
});

it('starts an upgrade when the previous upgrade status is older than the lock expiry', function () {
    Queue::fake();
    config([
        'app.env' => 'testing',
        'constants.tinyhelm.version' => '4.0.9',
        'constants.ssh.mux_enabled' => false,
    ]);
    updateTinyHelmTestCreateRootServerAndSettings();
    Http::fake(['*' => Http::response(['stable' => ['tinyhelm' => ['version' => '4.0.10']]], 200)]);
    updateTinyHelmTestActionWithStatus('3|Pulling Docker images|'.now()->subMinutes(16)->toIso8601String())
        ->handle(manual_update: true);

    expect(RemoteProcessCommand::read(Activity::query()->latest('id')->first()))
        ->toContain("bash /data/tinyhelm/source/upgrade.sh '4.0.10'");
});

it('backs up the instance database before the update', function () {
    updateTinyHelmTestPrepareUpdate();
    fakeRemoteProcesses(['*du -b*' => '128', '*' => '']);

    (new UpdateTinyHelm)->handle(manual_update: true);

    Process::assertRan(fn ($process) => str_contains(remoteProcessScript($process), 'pg_dump'));
    expect(RemoteProcessCommand::read(Activity::query()->latest('id')->first()))
        ->toContain("bash /data/tinyhelm/source/upgrade.sh '4.0.10'");
});

it('skips the backup before the update when it is disabled', function () {
    updateTinyHelmTestPrepareUpdate(['is_backup_before_update_enabled' => false]);
    Process::fake();

    (new UpdateTinyHelm)->handle(manual_update: true);

    Process::assertNotRan(fn ($process) => str_contains(remoteProcessScript($process), 'pg_dump'));
    expect(Activity::query()->count())->toBe(1);
});

it('does not start the update when the instance backup fails', function () {
    updateTinyHelmTestPrepareUpdate();
    fakeRemoteProcesses(['*pg_dump*' => Process::result(exitCode: 1), '*' => '']);

    expect(fn () => (new UpdateTinyHelm)->handle(manual_update: true))
        ->toThrow(Exception::class, 'The database backup failed, so the update was not started.')
        ->and(Activity::query()->count())->toBe(0);
});

it('saves the automatic update attempt before it queues the upgrade script', function () {
    Notification::fake();
    updateTinyHelmTestPrepareUpgrade();
    $savedTargetVersion = null;
    Activity::created(function () use (&$savedTargetVersion) {
        $savedTargetVersion = InstanceSettings::findOrFail(0)->auto_update_target_version;
    });

    updateTinyHelmTestActionWithStatus('')->handle();

    expect($savedTargetVersion)->toBe('4.0.10');
});

it('keeps the saved attempt of a running upgrade when an automatic update is skipped', function () {
    updateTinyHelmTestPrepareUpgrade();
    InstanceSettings::findOrFail(0)->update(['auto_update_target_version' => '4.0.8', 'auto_update_started_at' => now()->subMinutes(5)]);
    $action = updateTinyHelmTestActionWithStatus('3|Pulling Docker images|'.now()->subMinutes(2)->toIso8601String());

    expect(fn () => $action->handle())->toThrow(Exception::class, 'Another TinyHelm upgrade is already running');

    expect(InstanceSettings::findOrFail(0)->auto_update_target_version)->toBe('4.0.8');
});
