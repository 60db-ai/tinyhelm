<?php

/*
 * Parity fixtures: tokens with every ability/role combination the Rust API must answer exactly like
 * Laravel. Run with:  php artisan tinker --execute "require base_path('scripts/parity/fixtures.php');"
 * Writes plaintext tokens to .dev-instances/parity-tokens.json (gitignored). Idempotent: old parity
 * tokens are deleted first. Uses the first owner of team 1 as the owner; creates a member and an
 * outsider (owner of their own personal team) when missing.
 */

use App\Models\PersonalAccessToken;
use App\Models\Team;
use App\Models\User;
use Illuminate\Support\Facades\Hash;
use Illuminate\Support\Str;

$team = Team::query()->findOrFail(1);
$owner = $team->members()->wherePivotIn('role', ['owner', 'admin'])->firstOrFail();

$member = User::query()->firstOrCreate(
    ['email' => 'parity-member@tinyhelm.test'],
    ['name' => 'Parity Member', 'password' => Hash::make(Str::random(32))],
);
if (! $member->teams()->where('teams.id', $team->id)->exists()) {
    $member->teams()->attach($team->id, ['role' => 'member']);
}

$outsider = User::query()->firstOrCreate(
    ['email' => 'parity-outsider@tinyhelm.test'],
    ['name' => 'Parity Outsider', 'password' => Hash::make(Str::random(32))],
);
$outsiderTeam = $outsider->teams()->wherePivot('role', 'owner')->firstOrFail();

PersonalAccessToken::query()->where('name', 'like', 'parity-%')->delete();

$make = fn (User $user, string $name, array $abilities, int $teamId): string => $user->createToken('parity-'.$name, $abilities, null, $teamId)->plainTextToken;

$tokens = [
    'owner_root' => $make($owner, 'owner-root', ['root'], $team->id),
    'owner_star' => $make($owner, 'owner-star', ['*'], $team->id),
    'owner_read' => $make($owner, 'owner-read', ['read'], $team->id),
    'owner_read_sensitive' => $make($owner, 'owner-read-sensitive', ['read', 'read:sensitive'], $team->id),
    'owner_write' => $make($owner, 'owner-write', ['read', 'write', 'delete'], $team->id),
    'owner_deploy' => $make($owner, 'owner-deploy', ['read', 'deploy'], $team->id),
    'member_read_write' => $make($member, 'member-read-write', ['read', 'write'], $team->id),
    'outsider_root' => $make($outsider, 'outsider-root', ['root'], $outsiderTeam->id),
];

$dir = base_path('.dev-instances');
if (! is_dir($dir)) {
    mkdir($dir, 0700, true);
}
file_put_contents($dir.'/parity-tokens.json', json_encode($tokens, JSON_PRETTY_PRINT));
chmod($dir.'/parity-tokens.json', 0600);

echo 'parity tokens written for '.count($tokens)." roles\n";
