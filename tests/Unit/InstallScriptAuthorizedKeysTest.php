<?php

use Symfony\Component\Process\Process;

it('removes only previous TinyHelm keys from authorized_keys', function (string $path) {
    $script = file_get_contents(dirname(__DIR__, 2).'/'.$path);
    preg_match('/^\s*(sed -i .*~\/\.ssh\/authorized_keys)$/m', $script, $matches);
    expect($matches)->toHaveKey(1);

    $home = sys_get_temp_dir().'/tinyhelm-install-keys-'.bin2hex(random_bytes(4));
    mkdir($home.'/.ssh', 0700, true);
    file_put_contents($home.'/.ssh/authorized_keys', implode("\n", [
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey tinyhelm-disposable',
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey operator-lab',
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey me@tinyhelm-laptop',
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey tinyhelm-ops key',
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey admin@root@tinyhelm.example.com',
        'no-pty ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOptions deploy@tinyhelm.example.com',
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOldTinyHelmKey tinyhelm',
        // Installers before September 2024 generated their key with the comment "root@tinyhelm".
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOlderTinyHelmKey root@tinyhelm',
        '',
    ]));

    $process = new Process(['bash', '-c', $matches[1]], null, ['HOME' => $home]);
    $process->mustRun();

    expect(file_get_contents($home.'/.ssh/authorized_keys'))->toBe(implode("\n", [
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey tinyhelm-disposable',
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey operator-lab',
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey me@tinyhelm-laptop',
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey tinyhelm-ops key',
        'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorKey admin@root@tinyhelm.example.com',
        'no-pty ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOptions deploy@tinyhelm.example.com',
        '',
    ]));

    exec('rm -rf '.escapeshellarg($home));
})->with([
    'stable install script' => ['scripts/install.sh'],
]);
