<?php

use App\Jobs\ServerManagerJob;
use Tests\TestCase;

uses(TestCase::class);

it('lets Laravel run the server jobs when TINYHELM_RUST_JOBS is empty', function () {
    config()->set('constants.tinyhelm.rust_jobs', '');

    expect(ServerManagerJob::rustOwns('server-connection-check'))->toBeFalse()
        ->and(ServerManagerJob::rustOwns('server-storage-check'))->toBeFalse();
});

it('hands only the listed jobs to Rust', function () {
    config()->set('constants.tinyhelm.rust_jobs', ' server-connection-check , other');

    expect(ServerManagerJob::rustOwns('server-connection-check'))->toBeTrue()
        ->and(ServerManagerJob::rustOwns('server-storage-check'))->toBeFalse();
});

it('treats server-manager as both server checks', function () {
    config()->set('constants.tinyhelm.rust_jobs', 'server-manager');

    expect(ServerManagerJob::rustOwns('server-connection-check'))->toBeTrue()
        ->and(ServerManagerJob::rustOwns('server-storage-check'))->toBeTrue()
        ->and(ServerManagerJob::rustOwns('server-patch-check'))->toBeFalse();
});
