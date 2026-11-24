<?php

/*
 * Golden data for the Rust port of parseCommandsByLineForSudo (bootstrap/helpers/sudo.php).
 * Collects string literals from the sudo tests and app/Actions, runs each through the PHP parser
 * for a non-root server (user "ubuntu"), and writes {input, output} pairs to
 * rust-backend/tests/fixtures/sudo_golden.json.
 *   php artisan tinker --execute 'require base_path("scripts/parity/sudo-golden.php");'
 */

use App\Models\Server;

$files = array_merge(
    [base_path('tests/Unit/ParseCommandsByLineForSudoTest.php'), base_path('tests/Unit/SudoOwnershipSymlinkTest.php')],
    iterator_to_array(new RegexIterator(new RecursiveIteratorIterator(new RecursiveDirectoryIterator(base_path('app/Actions'))), '/\.php$/'), false),
);

$candidates = [];
foreach ($files as $file) {
    $file = is_string($file) ? $file : $file->getPathname();
    foreach (token_get_all(file_get_contents($file)) as $token) {
        if (! is_array($token) || $token[0] !== T_CONSTANT_ENCAPSED_STRING) {
            continue;
        }
        $raw = $token[1];
        if ($raw[0] === '"') {
            if (preg_match('/(?<!\\\\)\$[A-Za-z_{]/', $raw)) {
                continue; // interpolated
            }
            $value = stripcslashes(substr($raw, 1, -1));
        } else {
            $value = str_replace(["\\'", '\\\\'], ["'", '\\'], substr($raw, 1, -1));
        }
        if (strlen($value) < 3 || str_contains($value, "\n")) {
            continue;
        }
        $candidates[$value] = true;
    }
}

$server = new Server(['user' => 'ubuntu']);
$pairs = [];
foreach (array_keys($candidates) as $input) {
    $pairs[] = ['input' => $input, 'output' => parseCommandsByLineForSudo(collect([$input]), $server)[0], 'line_output' => parseLineForSudo($input, $server)];
}

file_put_contents(base_path('rust-backend/tests/fixtures/sudo_golden.json'), json_encode($pairs, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE));
echo count($pairs)." golden pairs\n";
