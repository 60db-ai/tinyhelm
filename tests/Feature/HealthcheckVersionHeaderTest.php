<?php

use App\Http\Controllers\Api\OtherController;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\Request;

uses(RefreshDatabase::class);

it('does not expose the running version on the healthcheck response', function () {
    config(['constants.tinyhelm.version' => '4.3.1']);

    $response = (new OtherController)->healthcheck(Request::create('/api/health', 'GET'));

    expect($response->getContent())->toBe('OK')
        ->and($response->headers->has('X-TinyHelm-Version'))->toBeFalse();
});

it('does not expose the running TinyHelm version on the public health endpoint', function () {
    config(['constants.tinyhelm.version' => '4.3.1']);

    $this->get('/api/health')
        ->assertSuccessful()
        ->assertSee('OK')
        ->assertHeaderMissing('X-TinyHelm-Version');
});
