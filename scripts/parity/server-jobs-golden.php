<?php

/*
 * Golden data for the Rust port of the server status jobs (rust-backend/src/jobs/).
 * Writes the exact remote commands, the per-server cron expressions and the backoff decisions
 * Laravel uses, for a fake root server and a fake non-root server, to
 * rust-backend/tests/fixtures/server_jobs_golden.json.
 *   php artisan tinker --execute 'require base_path("scripts/parity/server-jobs-golden.php");'
 */

use App\Jobs\ServerConnectionCheckJob;
use App\Jobs\ServerManagerJob;
use App\Models\Server;

$commands = [
    'probe' => ServerConnectionCheckJob::probeCommand(),
    'docker_version' => remoteCommandWithTimeout('docker version --format json', ServerConnectionCheckJob::DOCKER_TIMEOUT_SECONDS - 5),
    'compose_version' => remoteCommandWithTimeout('docker compose version --short', ServerConnectionCheckJob::DOCKER_TIMEOUT_SECONDS - 5),
    'storage' => 'df / --output=pcent | tr -cd 0-9',
];

$out = ['commands' => [], 'non_root_commands' => [], 'crons' => [], 'versions' => [], 'backoff' => []];
$nonRoot = new Server(['user' => 'ubuntu']);
foreach ($commands as $name => $command) {
    $out['commands'][$name] = $command;
    $out['non_root_commands'][$name] = parseCommandsByLineForSudo(collect([$command]), $nonRoot)[0];
}

foreach ([0, 1, 59, 60, 61, 1199, 1200, 1234] as $id) {
    $server = new Server;
    $server->id = $id;
    $out['crons'][] = [
        'id' => $id,
        'sentinel_version_check' => ServerManagerJob::sentinelVersionCheckCron($server),
        'image_rescan' => ServerManagerJob::imageRescanCron($server),
        'image_scan_digest' => ServerManagerJob::imageScanDigestCron($id),
    ];
}

foreach (['Docker version 27.3.1, build ce12230', '27.3', 'v2.29.7', '2.29.7-desktop.1', '', 'nothing'] as $raw) {
    $out['versions'][] = ['raw' => $raw, 'parsed' => parseDockerEngineVersion($raw)];
}

// shouldSkipDueToBackoff for self-hosted (cycle period 1 minute).
$backoff = new ReflectionMethod(ServerManagerJob::class, 'shouldSkipDueToBackoff');
$time = new ReflectionProperty(ServerManagerJob::class, 'executionTime');
$job = new ServerManagerJob;
foreach ([1, 2, 7, 42, 999] as $id) {
    foreach ([0, 2, 3, 5, 6, 11, 12, 40] as $count) {
        foreach ([0, 1, 2, 3, 7, 13, 59] as $minute) {
            $server = new Server;
            $server->id = $id;
            $server->unreachable_count = $count;
            $time->setValue($job, now()->setTime(10, $minute));
            $out['backoff'][] = ['id' => $id, 'count' => $count, 'minute' => $minute, 'skip' => $backoff->invoke($job, $server)];
        }
    }
}

file_put_contents(base_path('rust-backend/tests/fixtures/server_jobs_golden.json'), json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES));
echo "server jobs golden written\n";
