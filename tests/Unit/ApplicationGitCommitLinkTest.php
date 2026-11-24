<?php

use App\Models\Application;
use App\Models\GithubApp;

it('generates commit links for direct repository remotes', function (string $repository, string $expected) {
    $application = new Application;
    $application->setRelation('source', null);
    $application->git_repository = $repository;

    expect($application->gitCommitLink('1234567890abcdef'))->toBe($expected);
})->with([
    'HTTPS remote' => [
        'https://github.com/coollabsio/tinyhelm.git',
        'https://github.com/coollabsio/tinyhelm/commit/1234567890abcdef',
    ],
    'SSH remote' => [
        'git@github.com:coollabsio/tinyhelm.git',
        'https://github.com/coollabsio/tinyhelm/commit/1234567890abcdef',
    ],
    'SSH remote with custom username' => [
        'custom-user@git.example.com:coollabsio/tinyhelm.git',
        'https://git.example.com/coollabsio/tinyhelm/commit/1234567890abcdef',
    ],
    'SSH remote with custom username and port' => [
        'custom-user@git.example.com:2222/coollabsio/tinyhelm.git',
        'https://git.example.com/coollabsio/tinyhelm/commit/1234567890abcdef',
    ],
    'SSH URL' => [
        'ssh://git@gitlab.com/coollabsio/tinyhelm.git',
        'https://gitlab.com/coollabsio/tinyhelm/commit/1234567890abcdef',
    ],
    'Bitbucket HTTPS remote' => [
        'https://bitbucket.org/coollabsio/tinyhelm.git',
        'https://bitbucket.org/coollabsio/tinyhelm/commits/1234567890abcdef',
    ],
]);

it('does not generate commit links from incomplete repository URLs', function (string $repository) {
    $application = new Application;
    $application->setRelation('source', null);
    $application->git_repository = $repository;

    expect($application->gitCommitLink('1234567890abcdef'))->toBeNull();
})->with([
    'missing host' => 'https://',
    'missing scheme' => 'github.com/coollabsio/tinyhelm',
    'git protocol' => 'git://git.example.com/coollabsio/tinyhelm.git',
]);

it('converts ssh remotes with generic usernames into https repository links', function (string $repository, string $expectedBranch, string $expectedCommits, string $expectedWebhook) {
    $application = new Application;
    $application->setRelation('source', null);
    $application->git_repository = $repository;
    $application->git_branch = 'main';
    $application->base_directory = '/';

    expect($application->gitBranchLocation)->toBe($expectedBranch)
        ->and($application->gitCommits)->toBe($expectedCommits)
        ->and($application->gitWebhook)->toBe($expectedWebhook);
})->with([
    'git username' => [
        'git@github.com:coollabsio/tinyhelm.git',
        'https://github.com/coollabsio/tinyhelm/tree/main/',
        'https://github.com/coollabsio/tinyhelm/commits/main',
        'https://github.com/coollabsio/tinyhelm/settings/hooks',
    ],
    'custom username' => [
        'custom-user@git.example.com:organization/repository.git',
        'https://git.example.com/organization/repository/tree/main/',
        'https://git.example.com/organization/repository/commits/main',
        'https://git.example.com/organization/repository/settings/hooks',
    ],
    'custom username and port' => [
        'custom-user@git.example.com:2222/organization/repository.git',
        'https://git.example.com/organization/repository/tree/main/',
        'https://git.example.com/organization/repository/commits/main',
        'https://git.example.com/organization/repository/settings/hooks',
    ],
    'ssh scheme with port' => [
        'ssh://git@gitea.example.com:2222/organization/repository.git',
        'https://gitea.example.com/organization/repository/tree/main/',
        'https://gitea.example.com/organization/repository/commits/main',
        'https://gitea.example.com/organization/repository/settings/hooks',
    ],
]);

it('links the raw repository only when it is an http or https URL', function (string $repository, ?string $expected) {
    $application = new Application;
    $application->setRelation('source', null);
    $application->git_repository = $repository;
    $application->git_branch = 'main';

    expect($application->gitBranchLocation)->toBe($expected)
        ->and($application->gitCommits)->toBe($expected)
        ->and($application->gitWebhook)->toBe($expected);
})->with([
    'https URL' => ['https://git.example.com/coollabsio/tinyhelm', 'https://git.example.com/coollabsio/tinyhelm'],
    'other scheme' => ['ftp://git.example.com/coollabsio/tinyhelm', null],
    'owner and name' => ['coollabsio/tinyhelm', null],
]);

it('links full repository URLs of applications with a git source', function (string $repository, string $expectedBranch, string $expectedCommits, string $expectedWebhook, string $expectedCommit) {
    $application = new Application;
    $application->setRelation('source', new GithubApp(['html_url' => 'https://github.com']));
    $application->git_repository = $repository;
    $application->git_branch = 'main';
    $application->base_directory = '/';

    expect($application->gitBranchLocation)->toBe($expectedBranch)
        ->and($application->gitCommits)->toBe($expectedCommits)
        ->and($application->gitWebhook)->toBe($expectedWebhook)
        ->and($application->gitCommitLink('abc1234'))->toBe($expectedCommit);
})->with([
    'owner and name' => [
        'coollabsio/tinyhelm',
        'https://github.com/coollabsio/tinyhelm/tree/main/',
        'https://github.com/coollabsio/tinyhelm/commits/main',
        'https://github.com/coollabsio/tinyhelm/settings/hooks',
        'https://github.com/coollabsio/tinyhelm/commit/abc1234',
    ],
    'SSH remote' => [
        'git@github.com:coollabsio/tinyhelm-examples-deploy-key.git',
        'https://github.com/coollabsio/tinyhelm-examples-deploy-key/tree/main/',
        'https://github.com/coollabsio/tinyhelm-examples-deploy-key/commits/main',
        'https://github.com/coollabsio/tinyhelm-examples-deploy-key/settings/hooks',
        'https://github.com/coollabsio/tinyhelm-examples-deploy-key/commit/abc1234',
    ],
    'https URL with credentials' => [
        'https://user:token@gitlab.com/coollabsio/tinyhelm.git',
        'https://gitlab.com/coollabsio/tinyhelm/tree/main/',
        'https://gitlab.com/coollabsio/tinyhelm/commits/main',
        'https://gitlab.com/coollabsio/tinyhelm/settings/hooks',
        'https://gitlab.com/coollabsio/tinyhelm/commit/abc1234',
    ],
]);
