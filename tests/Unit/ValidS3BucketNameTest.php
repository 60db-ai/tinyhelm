<?php

use App\Rules\ValidS3BucketName;

function validS3BucketNameRulePasses(string $bucket): bool
{
    $failed = false;

    (new ValidS3BucketName)->validate('bucket', $bucket, function () use (&$failed) {
        $failed = true;
    });

    return ! $failed;
}

it('accepts valid s3 bucket names', function (string $bucket) {
    expect(validS3BucketNameRulePasses($bucket))->toBeTrue("Expected accepted: {$bucket}");
})->with([
    'short' => ['abc'],
    'simple' => ['tinyhelm-backups'],
    'legacy uppercase' => ['TinyHelmBackups'],
    'dots' => ['tinyhelm.backups'],
    'digits' => ['backup-123'],
    'max length' => [str_repeat('a', 63)],
]);

it('rejects invalid s3 bucket names and injection payloads', function (string $bucket) {
    expect(validS3BucketNameRulePasses($bucket))->toBeFalse("Expected rejected: {$bucket}");
})->with([
    'too short' => ['ab'],
    'too long' => [str_repeat('a', 64)],
    'underscore' => ['tinyhelm_backups'],
    'leading hyphen' => ['-tinyhelm-backups'],
    'trailing hyphen' => ['tinyhelm-backups-'],
    'leading dot' => ['.tinyhelm-backups'],
    'trailing dot' => ['tinyhelm-backups.'],
    'consecutive dots' => ['tinyhelm..backups'],
    'dot hyphen' => ['tinyhelm.-backups'],
    'hyphen dot' => ['tinyhelm-.backups'],
    'ipv4 address' => ['192.168.1.1'],
    'semicolon injection' => ['lab; id; #'],
    'command substitution' => ['lab$(id)'],
    'backticks' => ['lab`id`'],
    'pipe' => ['lab|id'],
    'ampersand' => ['lab&id'],
    'space' => ['lab bucket'],
    'newline' => ["lab\nid"],
]);
