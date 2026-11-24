# Rust migration plan: coolify (PHP)

1 package, 2389 source files (309463 lines), 1522 test files, 0 UI files, grouped into 277 units.

| Ready | Caution | Blocked | Keep in source language |
| ---: | ---: | ---: | ---: |
| 118 | 14 | 145 | 0 |

> 2 files had parse errors; their units are marked caution.

## Start here

No unit is a safe first port yet. See the blocked units and dependencies below.

## Order by package

Packages come after the workspace packages they use. Inside a package, port by wave: a unit's imports are in earlier waves.

### 1. 60db-ai/tinyhelm (`.`, portable, 1522 test files)

| Wave | Unit | Status | Lines | Readiness | Bridge | Main risks |
| ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | `app/Auth/Oidc/Exceptions/` | ready | 16 | 100 | PHP FFI (C ABI) | Class inheritance (5) |
| 0 | `app/Support/Metrics/` | caution | 33 | 100 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (2); Null references (1) |
| 0 | `bootstrap/` | blocked | 52 | 100 | PHP FFI (C ABI) | Dynamic code (eval, new Function, computed import) (1) |
| 0 | `lang/en/` | ready | 18 | 100 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (1) |
| 0 | `resources/views/components/applications/` | ready | 121 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/auth/` | ready | 101 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/dashboard/` | ready | 146 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/database/` | ready | 114 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/deployment/` | ready | 110 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/emails/` | ready | 23 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/security/` | ready | 51 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/service-database/` | ready | 20 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/service/` | ready | 87 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/services/` | ready | 39 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/settings/` | ready | 33 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/skeleton/` | ready | 159 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/status/` | ready | 81 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/team/` | ready | 62 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/terminal/` | ready | 78 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/theme-controls/` | ready | 20 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/errors/` | ready | 110 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/invitation/` | ready | 32 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/admin/` | ready | 105 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/destination/new/` | ready | 28 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/dev/` | ready | 21 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/partials/` | ready | 163 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/application/partials/` | ready | 177 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/application/preview/` | ready | 25 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/clickhouse/` | ready | 121 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/dragonfly/` | ready | 100 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/keydb/` | ready | 111 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/mariadb/` | ready | 114 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/mongodb/` | ready | 111 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/mysql/` | ready | 114 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/postgresql/` | ready | 149 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/redis/` | ready | 120 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/sqlite/` | ready | 118 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/new/partials/` | ready | 97 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/partials/` | ready | 10 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/shared/partials/` | ready | 8 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/security/cloud-init-script/` | ready | 50 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/security/cloud-provider-token/` | ready | 60 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/security/partials/` | ready | 157 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/security/private-key/partials/` | ready | 79 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/analytics/` | ready | 18 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/ca-certificate/` | ready | 82 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/cloud-provider-token/` | ready | 72 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/new/partials/` | ready | 9 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/partials/` | ready | 126 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/private-key/` | ready | 119 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/sentinel/` | ready | 62 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/shared-variables/` | ready | 141 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/shared-variables/environment/` | ready | 68 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/shared-variables/project/` | ready | 71 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/shared-variables/server/` | ready | 53 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/shared-variables/team/` | ready | 7 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/shared/` | ready | 13 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/tags/partials/` | ready | 7 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/proxy/` | ready | 124 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/server/` | ready | 9 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/source/github/` | ready | 4 | 100 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/shared-variables/` | ready | 214 | 99 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/backup-edit/` | ready | 200 | 99 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/backup/` | ready | 202 | 99 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/service/partials/` | ready | 214 | 99 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/docker-registries/` | ready | 212 | 99 | PHP FFI (C ABI) | - |
| 0 | `resources/views/source/` | ready | 193 | 99 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/image-scan/` | ready | 239 | 98 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/table/` | ready | 236 | 98 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/notification/` | ready | 262 | 97 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/destination/` | ready | 288 | 97 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/application/backup/` | ready | 272 | 97 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/shared/scheduled-task/` | ready | 287 | 97 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/notifications/` | ready | 335 | 96 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/team/member/` | ready | 336 | 96 | PHP FFI (C ABI) | - |
| 0 | `app/Livewire/Notifications/Concerns/` | ready | 40 | 95 | PHP FFI (C ABI) | Callback-shaped APIs (2); Insertion-ordered maps and object keys (1) |
| 0 | `resources/views/livewire/project/shared/storages/volume-backups/` | ready | 350 | 95 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/security/private-key/` | ready | 357 | 95 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/proxy/` | ready | 376 | 95 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/settings/` | ready | 380 | 95 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/source/gitea/` | ready | 360 | 95 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/source/gitlab/` | ready | 355 | 95 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/terminal/` | ready | 343 | 95 | PHP FFI (C ABI) | - |
| 0 | `app/Services/DeploymentConfiguration/Concerns/` | ready | 26 | 94 | PHP FFI (C ABI) | String offsets and slicing (1); Null references (2) |
| 0 | `resources/views/components/application/` | ready | 390 | 94 | PHP FFI (C ABI) | - |
| 0 | `resources/views/emails/` | ready | 403 | 94 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/service/volume-backup/` | ready | 394 | 94 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/subscription/` | ready | 420 | 94 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/profile/` | ready | 447 | 93 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/shared/storages/` | ready | 430 | 93 | PHP FFI (C ABI) | - |
| 0 | `app/Auth/` | ready | 22 | 92 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (2); Class inheritance (1) |
| 0 | `app/Data/Traffic/` | caution | 131 | 92 | PHP FFI (C ABI) | Class inheritance (4); Null references (4) |
| 0 | `resources/views/livewire/dashboard/` | ready | 411 | 92 | PHP FFI (C ABI) | Callback-shaped APIs (1) |
| 0 | `resources/views/livewire/image-scan/partials/` | ready | 469 | 92 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/tags/` | ready | 466 | 92 | PHP FFI (C ABI) | - |
| 0 | `rector.php` | ready | 30 | 91 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (3) |
| 0 | `resources/views/auth/` | ready | 407 | 90 | PHP FFI (C ABI) | Dynamic typing (1); Reflection and prototypes (1) |
| 0 | `app/Support/Actions/` | blocked | 11 | 89 | PHP FFI (C ABI) | Class inheritance (1) |
| 0 | `resources/views/livewire/project/resource/` | ready | 586 | 89 | PHP FFI (C ABI) | - |
| 0 | `resources/views/components/server/` | ready | 640 | 88 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/boarding/` | ready | 647 | 88 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/source/github/` | ready | 665 | 87 | PHP FFI (C ABI) | - |
| 0 | `app/Livewire/Dev/` | blocked | 25 | 86 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (2); Class inheritance (1) |
| 0 | `resources/views/livewire/project/shared/environment-variable/` | ready | 723 | 86 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/team/` | ready | 707 | 86 | PHP FFI (C ABI) | - |
| 0 | `app/Notifications/Dto/` | caution | 136 | 85 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (9) |
| 0 | `resources/views/livewire/server/new/` | ready | 762 | 85 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/fleet/` | ready | 471 | 84 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (7); Callback-shaped APIs (1) |
| 0 | `app/Mcp/Prompts/` | ready | 108 | 83 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (3); Class inheritance (2) |
| 0 | `app/Casts/` | blocked | 43 | 82 | PHP FFI (C ABI) | Null references (5); JSON round-trips (2) |
| 0 | `resources/views/layouts/` | ready | 880 | 82 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/security/` | ready | 869 | 82 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/traffic/` | ready | 896 | 82 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/image-scan/` | ready | 902 | 81 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/application/deployment/` | ready | 903 | 81 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/server/security/` | ready | 910 | 81 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/database/` | ready | 1015 | 79 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/` | ready | 1057 | 78 | PHP FFI (C ABI) | - |
| 0 | `app/Support/DomainConnect/` | caution | 296 | 74 | PHP FFI (C ABI) | String offsets and slicing (3); Null references (10) |
| 0 | `app/Contracts/` | blocked | 30 | 72 | PHP FFI (C ABI) | Class inheritance (1) |
| 0 | `resources/views/components/forms/` | ready | 1354 | 70 | PHP FFI (C ABI) | - |
| 0 | `app/View/Components/Forms/` | blocked | 353 | 66 | PHP FFI (C ABI) | Null references (54); Insertion-ordered maps and object keys (5) |
| 0 | `resources/views/livewire/project/new/` | ready | 1747 | 60 | PHP FFI (C ABI) | - |
| 0 | `app/Console/Commands/Generate/` | blocked | 262 | 57 | PHP FFI (C ABI) | Regexes built at runtime (4); Insertion-ordered maps and object keys (19) |
| 0 | `resources/views/livewire/project/service/` | caution | 2307 | 46 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/shared/` | caution | 3257 | 23 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/project/application/` | caution | 3031 | 21 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (5); Callback-shaped APIs (1) |
| 0 | `app/Services/Security/` | blocked | 285 | 7 | PHP FFI (C ABI) | String offsets and slicing (17); Regexes built at runtime (9) |
| 0 | `resources/views/livewire/server/` | caution | 3965 | 5 | PHP FFI (C ABI) | - |
| 0 | `resources/views/livewire/` | caution | 3706 | 1 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (10); Callback-shaped APIs (2) |
| 0 | `resources/views/components/` | caution | 5226 | 0 | PHP FFI (C ABI) | - |
| 1 | `app/Logging/` | blocked | 79 | 85 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (5); Callback-shaped APIs (2) |
| 1 | `app/Notifications/Internal/` | blocked | 60 | 81 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (2); Class inheritance (2) |
| 1 | `app/Repositories/` | blocked | 40 | 78 | PHP FFI (C ABI) | Callback-shaped APIs (3); Class inheritance (1) |
| 2 | `app/Models/Scopes/` | blocked | 38 | 90 | PHP FFI (C ABI) | Callback-shaped APIs (1) |
| 2 | `app/Data/` | caution | 108 | 82 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (4); Callback-shaped APIs (3) |
| 2 | `app/Notifications/Container/` | blocked | 117 | 79 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (8); Null references (2) |
| 2 | `app/Notifications/TransactionalEmails/` | blocked | 131 | 77 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (7); Class inheritance (3) |
| 2 | `app/Actions/Fortify/` | blocked | 175 | 72 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (22); Null references (3) |
| 2 | `app/Services/GithubRunner/` | blocked | 329 | 72 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (12); Callback-shaped APIs (2) |
| 2 | `app/Notifications/ScheduledTask/` | blocked | 260 | 70 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (12); Null references (3) |
| 2 | `app/Notifications/Database/` | blocked | 382 | 69 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (12); Class inheritance (4) |
| 2 | `app/Actions/Team/` | blocked | 280 | 68 | PHP FFI (C ABI) | Callback-shaped APIs (19); Insertion-ordered maps and object keys (6) |
| 2 | `app/Auth/Oidc/Socialite/` | blocked | 246 | 64 | PHP FFI (C ABI) | Null references (19); Insertion-ordered maps and object keys (11) |
| 2 | `app/Notifications/VolumeBackup/` | blocked | 460 | 61 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (32); Null references (6) |
| 2 | `app/Enums/` | ready | 594 | 56 | PHP FFI (C ABI) | String offsets and slicing (1); Insertion-ordered maps and object keys (16) |
| 2 | `app/Notifications/` | blocked | 418 | 49 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (31); Class inheritance (6) |
| 2 | `app/Actions/User/` | blocked | 596 | 47 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (17); Callback-shaped APIs (13) |
| 2 | `app/Actions/Docker/` | blocked | 573 | 44 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (27); Callback-shaped APIs (15) |
| 2 | `app/Auth/Oidc/` | blocked | 366 | 43 | PHP FFI (C ABI) | String offsets and slicing (1); Null references (24) |
| 2 | `app/Notifications/Channels/` | blocked | 417 | 43 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (13); Null references (12) |
| 2 | `app/Http/Controllers/Webhook/Concerns/` | blocked | 548 | 42 | PHP FFI (C ABI) | String offsets and slicing (4); Null references (27) |
| 2 | `app/Notifications/Application/` | blocked | 718 | 41 | PHP FFI (C ABI) | String offsets and slicing (1); Insertion-ordered maps and object keys (40) |
| 2 | `app/Events/` | blocked | 577 | 38 | PHP FFI (C ABI) | Class inheritance (29); Insertion-ordered maps and object keys (18) |
| 2 | `app/Actions/TinyHelmTask/` | blocked | 332 | 36 | PHP FFI (C ABI) | String offsets and slicing (6); Regexes built at runtime (2) |
| 2 | `app/Services/DeploymentConfiguration/` | blocked | 709 | 35 | PHP FFI (C ABI) | Regexes built at runtime (2); Insertion-ordered maps and object keys (48) |
| 2 | `app/Exceptions/` | blocked | 364 | 32 | PHP FFI (C ABI) | Regexes built at runtime (2); String offsets and slicing (1) |
| 2 | `app/Services/Access/` | blocked | 645 | 32 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (77); Null references (50) |
| 2 | `app/Support/DatabaseImport/` | blocked | 628 | 30 | PHP FFI (C ABI) | String offsets and slicing (3); Null references (19) |
| 2 | `app/Notifications/ImageScan/` | blocked | 511 | 27 | PHP FFI (C ABI) | String offsets and slicing (13); Insertion-ordered maps and object keys (40) |
| 2 | `app/Policies/` | blocked | 2101 | 26 | PHP FFI (C ABI) | Null references (31); Insertion-ordered maps and object keys (2) |
| 2 | `app/Actions/Service/` | blocked | 721 | 19 | PHP FFI (C ABI) | String offsets and slicing (1); Insertion-ordered maps and object keys (51) |
| 2 | `app/Actions/Application/` | blocked | 597 | 12 | PHP FFI (C ABI) | String offsets and slicing (4); Insertion-ordered maps and object keys (31) |
| 2 | `app/Providers/` | blocked | 741 | 9 | PHP FFI (C ABI) | String offsets and slicing (1); Dynamic code (eval, new Function, computed import) (1) |
| 2 | `app/Notifications/Server/` | blocked | 1279 | 8 | PHP FFI (C ABI) | String offsets and slicing (8); Insertion-ordered maps and object keys (67) |
| 2 | `app/Actions/Shared/` | blocked | 947 | 6 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (63); Callback-shaped APIs (15) |
| 2 | `app/Actions/Stripe/` | blocked | 1062 | 6 | PHP FFI (C ABI) | String offsets and slicing (3); Insertion-ordered maps and object keys (119) |
| 2 | `app/Http/Middleware/` | blocked | 1116 | 5 | PHP FFI (C ABI) | String offsets and slicing (5); Regexes built at runtime (1) |
| 2 | `app/Actions/Database/` | blocked | 3341 | 0 | PHP FFI (C ABI) | Regexes built at runtime (6); String offsets and slicing (1) |
| 2 | `app/Actions/ImageScan/` | blocked | 2543 | 0 | PHP FFI (C ABI) | String offsets and slicing (19); Regexes built at runtime (3) |
| 2 | `app/Actions/Proxy/` | blocked | 984 | 0 | PHP FFI (C ABI) | Regexes built at runtime (4); String offsets and slicing (3) |
| 2 | `app/Actions/Server/` | blocked | 3351 | 0 | PHP FFI (C ABI) | Regexes built at runtime (17); String offsets and slicing (5) |
| 2 | `app/Helpers/` | blocked | 1063 | 0 | PHP FFI (C ABI) | String offsets and slicing (4); Regexes built at runtime (4) |
| 2 | `app/Jobs/` | blocked | 19662 | 0 | PHP FFI (C ABI) | String offsets and slicing (27); Regexes built at runtime (24) |
| 2 | `app/Livewire/` | blocked | 3951 | 0 | PHP FFI (C ABI) | String offsets and slicing (35); Regexes built at runtime (18) |
| 2 | `app/Livewire/Concerns/` | blocked | 1344 | 0 | PHP FFI (C ABI) | String offsets and slicing (16); Insertion-ordered maps and object keys (91) |
| 2 | `app/Livewire/Server/` | blocked | 5249 | 0 | PHP FFI (C ABI) | String offsets and slicing (9); Regexes built at runtime (4) |
| 2 | `app/Models/` | blocked | 22091 | 0 | PHP FFI (C ABI) | String offsets and slicing (41); Regexes built at runtime (24) |
| 2 | `app/Rules/` | blocked | 1566 | 0 | PHP FFI (C ABI) | String offsets and slicing (34); Regexes built at runtime (13) |
| 2 | `app/Services/` | blocked | 8500 | 0 | PHP FFI (C ABI) | String offsets and slicing (43); Regexes built at runtime (23) |
| 2 | `app/Services/Dns/` | blocked | 1568 | 0 | PHP FFI (C ABI) | String offsets and slicing (27); Regexes built at runtime (3) |
| 2 | `app/Services/ImageScan/` | blocked | 8500 | 0 | PHP FFI (C ABI) | String offsets and slicing (73); Regexes built at runtime (9) |
| 2 | `app/Services/ServerTransfer/` | blocked | 6587 | 0 | PHP FFI (C ABI) | String offsets and slicing (16); Regexes built at runtime (4) |
| 2 | `app/Support/` | blocked | 3532 | 0 | PHP FFI (C ABI) | String offsets and slicing (46); Regexes built at runtime (22) |
| 2 | `app/Traits/` | blocked | 3791 | 0 | PHP FFI (C ABI) | String offsets and slicing (9); Regexes built at runtime (8) |
| 3 | `app/Actions/Destination/` | ready | 12 | 100 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (2) |
| 3 | `app/Livewire/Server/New/Concerns/` | ready | 26 | 100 | PHP FFI (C ABI) | - |
| 3 | `app/Livewire/Server/Analytics/` | blocked | 20 | 89 | PHP FFI (C ABI) | Class inheritance (2) |
| 3 | `app/Livewire/Storage/Concerns/` | ready | 63 | 89 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (5); Null references (4) |
| 3 | `app/Mcp/Servers/` | caution | 275 | 84 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (5); Null references (2) |
| 3 | `app/Livewire/Admin/` | blocked | 88 | 82 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (3); Class inheritance (1) |
| 3 | `app/View/Components/Services/` | blocked | 50 | 82 | PHP FFI (C ABI) | Callback-shaped APIs (4); Insertion-ordered maps and object keys (1) |
| 3 | `app/Livewire/Server/Sentinel/` | blocked | 54 | 79 | PHP FFI (C ABI) | Class inheritance (4); Insertion-ordered maps and object keys (2) |
| 3 | `app/Mcp/Resources/` | ready | 138 | 79 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (8); Class inheritance (5) |
| 3 | `app/Livewire/Destination/New/` | blocked | 97 | 78 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (7); Class inheritance (2) |
| 3 | `app/Livewire/Project/Application/Preview/` | blocked | 59 | 78 | PHP FFI (C ABI) | Class inheritance (2); Exceptions (3) |
| 3 | `app/Livewire/Security/CloudInitScript/` | blocked | 84 | 78 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (6); Class inheritance (2) |
| 3 | `app/Livewire/Server/CaCertificate/` | blocked | 120 | 77 | PHP FFI (C ABI) | Class inheritance (2); Null references (2) |
| 3 | `app/Http/` | blocked | 123 | 76 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (5); Class inheritance (1) |
| 3 | `app/Livewire/Server/PrivateKey/` | blocked | 115 | 76 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (4); Class inheritance (2) |
| 3 | `app/Livewire/Forms/` | caution | 93 | 75 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (14); Dynamic typing (7) |
| 3 | `app/Livewire/Terminal/` | blocked | 81 | 75 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (4); Callback-shaped APIs (3) |
| 3 | `app/Livewire/Security/CloudProviderToken/` | blocked | 167 | 73 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (6); Null references (4) |
| 3 | `app/Livewire/Server/CloudProviderToken/` | blocked | 197 | 73 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (16); Class inheritance (2) |
| 3 | `app/Livewire/Project/Database/Dragonfly/` | blocked | 219 | 70 | PHP FFI (C ABI) | Null references (10); Class inheritance (6) |
| 3 | `app/Livewire/SharedVariables/Team/` | blocked | 154 | 70 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (5); Class inheritance (3) |
| 3 | `app/Livewire/Project/Database/Keydb/` | blocked | 223 | 69 | PHP FFI (C ABI) | Null references (12); Class inheritance (6) |
| 3 | `app/Livewire/Project/Database/Mariadb/` | blocked | 240 | 69 | PHP FFI (C ABI) | Null references (12); Class inheritance (5) |
| 3 | `app/Livewire/Project/Database/Clickhouse/` | blocked | 247 | 68 | PHP FFI (C ABI) | Null references (14); Class inheritance (6) |
| 3 | `app/Livewire/SharedVariables/` | blocked | 142 | 67 | PHP FFI (C ABI) | Null references (10); Insertion-ordered maps and object keys (8) |
| 3 | `app/Livewire/Project/Database/Mongodb/` | blocked | 260 | 66 | PHP FFI (C ABI) | Null references (13); Insertion-ordered maps and object keys (8) |
| 3 | `app/Livewire/Project/Database/Mysql/` | blocked | 265 | 66 | PHP FFI (C ABI) | Null references (12); Insertion-ordered maps and object keys (8) |
| 3 | `app/Services/SecretManagers/` | blocked | 162 | 66 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (26); Callback-shaped APIs (4) |
| 3 | `app/Livewire/SharedVariables/Environment/` | blocked | 182 | 65 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (6); Callback-shaped APIs (5) |
| 3 | `app/Console/` | blocked | 183 | 64 | PHP FFI (C ABI) | String offsets and slicing (1); Dynamic code (eval, new Function, computed import) (1) |
| 3 | `app/Listeners/` | blocked | 376 | 64 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (22); Null references (22) |
| 3 | `app/Livewire/Destination/` | blocked | 243 | 62 | PHP FFI (C ABI) | String offsets and slicing (1); Insertion-ordered maps and object keys (14) |
| 3 | `app/Livewire/SharedVariables/Project/` | blocked | 173 | 61 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (6); Class inheritance (4) |
| 3 | `app/Livewire/Project/Application/Backup/` | blocked | 255 | 60 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (20); Null references (9) |
| 3 | `app/Livewire/Project/Database/Backup/` | blocked | 242 | 59 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (14); Callback-shaped APIs (8) |
| 3 | `app/Livewire/SharedVariables/Server/` | blocked | 183 | 59 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (11); Null references (6) |
| 3 | `app/Livewire/Dashboard/` | blocked | 285 | 57 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (14); Class inheritance (6) |
| 3 | `app/Livewire/Boarding/` | blocked | 439 | 56 | PHP FFI (C ABI) | Null references (29); Insertion-ordered maps and object keys (27) |
| 3 | `app/Livewire/Project/Database/Sqlite/` | blocked | 346 | 56 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (19); Null references (15) |
| 3 | `app/Livewire/Project/Database/Postgresql/` | blocked | 400 | 54 | PHP FFI (C ABI) | Null references (18); Insertion-ordered maps and object keys (17) |
| 3 | `app/Livewire/Subscription/` | blocked | 451 | 54 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (16); Null references (15) |
| 3 | `app/Livewire/Project/Service/VolumeBackup/` | blocked | 330 | 53 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (22); Null references (19) |
| 3 | `app/Livewire/Source/Gitlab/` | blocked | 376 | 51 | PHP FFI (C ABI) | String offsets and slicing (1); Insertion-ordered maps and object keys (17) |
| 3 | `app/Livewire/Project/Resource/` | blocked | 380 | 50 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (32); Null references (13) |
| 3 | `app/Livewire/Source/Gitea/` | blocked | 390 | 50 | PHP FFI (C ABI) | String offsets and slicing (1); Insertion-ordered maps and object keys (18) |
| 3 | `app/Actions/IntegrationToken/` | blocked | 228 | 49 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (26); Callback-shaped APIs (9) |
| 3 | `app/Livewire/Fleet/` | blocked | 359 | 49 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (53); Null references (26) |
| 3 | `app/Livewire/Project/Application/Deployment/` | caution | 586 | 49 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (39); Null references (17) |
| 3 | `app/Livewire/Server/DockerRegistries/` | blocked | 264 | 49 | PHP FFI (C ABI) | Regexes built at runtime (3); Insertion-ordered maps and object keys (31) |
| 3 | `app/Livewire/Source/Github/` | blocked | 512 | 48 | PHP FFI (C ABI) | String offsets and slicing (2); Insertion-ordered maps and object keys (22) |
| 3 | `app/Livewire/Project/Shared/ScheduledTask/` | blocked | 550 | 47 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (21); Class inheritance (8) |
| 3 | `app/Livewire/Team/Member/` | blocked | 312 | 47 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (54); Callback-shaped APIs (26) |
| 3 | `database/factories/` | blocked | 495 | 47 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (33); Class inheritance (22) |
| 3 | `app/Livewire/Profile/` | blocked | 380 | 44 | PHP FFI (C ABI) | String offsets and slicing (2); Insertion-ordered maps and object keys (24) |
| 3 | `app/Livewire/Security/PrivateKey/` | blocked | 483 | 42 | PHP FFI (C ABI) | Regexes built at runtime (1); Insertion-ordered maps and object keys (39) |
| 3 | `app/Livewire/Settings/` | blocked | 705 | 36 | PHP FFI (C ABI) | String offsets and slicing (1); Insertion-ordered maps and object keys (32) |
| 3 | `app/Services/Auth/` | blocked | 439 | 35 | PHP FFI (C ABI) | String offsets and slicing (7); Insertion-ordered maps and object keys (26) |
| 3 | `app/Livewire/Server/Proxy/` | blocked | 500 | 34 | PHP FFI (C ABI) | String offsets and slicing (1); Regexes built at runtime (1) |
| 3 | `app/Livewire/Project/` | blocked | 834 | 29 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (58); Class inheritance (29) |
| 3 | `scripts/transfer-test/` | blocked | 1049 | 25 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (181); Callback-shaped APIs (23) |
| 3 | `app/Actions/Development/` | blocked | 725 | 23 | PHP FFI (C ABI) | String offsets and slicing (2); Regexes built at runtime (2) |
| 3 | `app/Mcp/Concerns/` | blocked | 674 | 16 | PHP FFI (C ABI) | String offsets and slicing (5); Regexes built at runtime (2) |
| 3 | `app/Livewire/Notifications/` | blocked | 1581 | 12 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (205); Null references (48) |
| 3 | `database/seeders/` | blocked | 1161 | 10 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (104); Class inheritance (26) |
| 3 | `app/Console/Commands/Cloud/` | blocked | 1505 | 1 | PHP FFI (C ABI) | String offsets and slicing (1); Insertion-ordered maps and object keys (63) |
| 3 | `app/Livewire/ImageScan/` | blocked | 2053 | 0 | PHP FFI (C ABI) | String offsets and slicing (1); Insertion-ordered maps and object keys (209) |
| 3 | `app/Livewire/Project/Database/` | blocked | 1855 | 0 | PHP FFI (C ABI) | Regexes built at runtime (2); Insertion-ordered maps and object keys (88) |
| 3 | `app/Livewire/Project/New/` | blocked | 1960 | 0 | PHP FFI (C ABI) | String offsets and slicing (2); Insertion-ordered maps and object keys (109) |
| 3 | `app/Livewire/Project/Shared/EnvironmentVariable/` | blocked | 1861 | 0 | PHP FFI (C ABI) | Regexes built at runtime (2); String offsets and slicing (1) |
| 3 | `app/Mcp/Tools/` | blocked | 6690 | 0 | PHP FFI (C ABI) | String offsets and slicing (26); Regexes built at runtime (1) |
| 3 | `bootstrap/helpers/` | blocked | 18126 | 0 | PHP FFI (C ABI) | String offsets and slicing (153); Regexes built at runtime (95) |
| 3 | `config/` | blocked | 2596 | 0 | PHP FFI (C ABI) | String offsets and slicing (2); Insertion-ordered maps and object keys (255) |
| 4 | `public/` | blocked | 44 | 87 | PHP FFI (C ABI) | Dynamic code (eval, new Function, computed import) (1) |
| 4 | `app/Livewire/Project/Database/Redis/` | blocked | 242 | 66 | PHP FFI (C ABI) | Null references (13); Insertion-ordered maps and object keys (9) |
| 4 | `scripts/` | blocked | 257 | 59 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (24); Integer width and overflow (18) |
| 4 | `app/Livewire/Storage/` | blocked | 742 | 36 | PHP FFI (C ABI) | Regexes built at runtime (1); Insertion-ordered maps and object keys (31) |
| 4 | `app/Livewire/Team/` | blocked | 770 | 24 | PHP FFI (C ABI) | String offsets and slicing (2); Insertion-ordered maps and object keys (38) |
| 4 | `app/Http/Controllers/` | blocked | 540 | 23 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (31); Null references (20) |
| 4 | `app/Livewire/Server/Security/` | blocked | 829 | 10 | PHP FFI (C ABI) | Regexes built at runtime (2); String offsets and slicing (1) |
| 4 | `app/Console/Commands/` | blocked | 5162 | 0 | PHP FFI (C ABI) | Regexes built at runtime (16); String offsets and slicing (6) |
| 4 | `app/Livewire/Server/New/` | blocked | 2106 | 0 | PHP FFI (C ABI) | String offsets and slicing (8); Regexes built at runtime (1) |
| 4 | `database/migrations/` | blocked | 17276 | 0 | PHP FFI (C ABI) | String offsets and slicing (4); Callback-shaped APIs (1243) |
| 5 | `app/Http/Controllers/Api/Concerns/` | blocked | 325 | 35 | PHP FFI (C ABI) | String offsets and slicing (4); Insertion-ordered maps and object keys (62) |
| 5 | `app/Livewire/Security/` | blocked | 1092 | 17 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (116); Callback-shaped APIs (20) |
| 5 | `app/Http/Controllers/Api/` | blocked | 34006 | 0 | PHP FFI (C ABI) | String offsets and slicing (29); Regexes built at runtime (4) |
| 5 | `app/Http/Controllers/Webhook/` | blocked | 2527 | 0 | PHP FFI (C ABI) | String offsets and slicing (2); Insertion-ordered maps and object keys (194) |
| 5 | `app/Livewire/Project/Shared/` | blocked | 3711 | 0 | PHP FFI (C ABI) | String offsets and slicing (6); Regexes built at runtime (4) |
| 6 | `app/Livewire/Tags/` | blocked | 254 | 52 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (14); Callback-shaped APIs (9) |
| 6 | `app/Livewire/Project/Shared/Storages/` | blocked | 801 | 36 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (61); Null references (13) |
| 6 | `app/Livewire/Project/Service/` | blocked | 4002 | 0 | PHP FFI (C ABI) | String offsets and slicing (16); Insertion-ordered maps and object keys (193) |
| 7 | `app/Livewire/Project/Application/` | blocked | 5418 | 0 | PHP FFI (C ABI) | String offsets and slicing (15); Regexes built at runtime (2) |
| 8 | `routes/` | blocked | 1179 | 21 | PHP FFI (C ABI) | Insertion-ordered maps and object keys (879); Callback-shaped APIs (51) |

## Composer dependencies

| Package | Units | Rust | Fit | Note |
| --- | ---: | --- | --- | --- |
| `laravel/framework` | 144 | axum | none | a web framework is an application rewrite, not a port |
| `Livewire` | 69 | ? | unknown |  |
| `Laravel` | 29 | ? | unknown |  |
| `Spatie` | 23 | ? | unknown |  |
| `Lorisleiva` | 12 | ? | unknown |  |
| `Stripe` | 7 | ? | unknown |  |
| `symfony/http-kernel` | 7 | ? | unknown |  |
| `Cron` | 5 | ? | unknown |  |
| `phpseclib3` | 4 | ? | unknown |  |
| `DanHarrin` | 3 | ? | unknown |  |
| `SocialiteProviders` | 3 | ? | unknown |  |
| `Aws` | 2 | ? | unknown |  |
| `OpenApi` | 2 | ? | unknown |  |
| `Pion` | 2 | ? | unknown |  |
| `Stevebauman` | 2 | ? | unknown |  |
| `symfony/mailer` | 2 | ? | unknown |  |
| `symfony/mime` | 2 | ? | unknown |  |
| `Lcobucci` | 1 | ? | unknown |  |
| `Nubs` | 1 | ? | unknown |  |
| `Pdo` | 1 | ? | unknown |  |
| `Poliander` | 1 | ? | unknown |  |
| `Rector` | 1 | ? | unknown |  |
| `RectorLaravel` | 1 | ? | unknown |  |
| `Resend` | 1 | ? | unknown |  |
| `Termwind` | 1 | ? | unknown |  |
| `Visus` | 1 | ? | unknown |  |
| `symfony/http-foundation` | 9 | http, axum | partial |  |
| `league/flysystem` | 3 | object_store, opendal | partial |  |
| `psr/http-message` | 2 | http, axum | partial |  |
| `nesbot/carbon` | 17 | chrono, jiff | good |  |
| `symfony/yaml` | 16 | serde_yaml | good |  |
| `guzzlehttp/guzzle` | 6 | reqwest | good |  |
| `monolog/monolog` | 2 | tracing, log | good |  |
| `symfony/process` | 2 | std::process | good |  |
| `firebase/php-jwt` | 1 | jsonwebtoken | good |  |
| `psr/log` | 1 | tracing, log | good |  |
| `symfony/finder` | 1 | walkdir, globset | good |  |

## Blocked

- `app/Actions/Application/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Database/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Development/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Docker/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Fortify/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/ImageScan/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/IntegrationToken/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Proxy/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Server/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Service/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Shared/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Stripe/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/Team/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/TinyHelmTask/`: `laravel/framework` has no Rust equivalent.
- `app/Actions/User/`: `laravel/framework` has no Rust equivalent.
- `app/Auth/Oidc/`: `laravel/framework` has no Rust equivalent.
- `app/Auth/Oidc/Socialite/`: `laravel/framework` has no Rust equivalent.
- `app/Casts/`: `laravel/framework` has no Rust equivalent.
- `app/Console/`: `laravel/framework` has no Rust equivalent.
- `app/Console/Commands/`: `laravel/framework` has no Rust equivalent.
- `app/Console/Commands/Cloud/`: `laravel/framework` has no Rust equivalent.
- `app/Console/Commands/Generate/`: `laravel/framework` has no Rust equivalent.
- `app/Contracts/`: `laravel/framework` has no Rust equivalent.
- `app/Events/`: `laravel/framework` has no Rust equivalent.
- `app/Exceptions/`: `laravel/framework` has no Rust equivalent.
- `app/Helpers/`: `laravel/framework` has no Rust equivalent.
- `app/Http/`: `laravel/framework` has no Rust equivalent.
- `app/Http/Controllers/`: `laravel/framework` has no Rust equivalent.
- `app/Http/Controllers/Api/`: `laravel/framework` has no Rust equivalent.
- `app/Http/Controllers/Api/Concerns/`: `laravel/framework` has no Rust equivalent.
- `app/Http/Controllers/Webhook/`: `laravel/framework` has no Rust equivalent.
- `app/Http/Controllers/Webhook/Concerns/`: `laravel/framework` has no Rust equivalent.
- `app/Http/Middleware/`: `laravel/framework` has no Rust equivalent.
- `app/Jobs/`: `laravel/framework` has no Rust equivalent.
- `app/Listeners/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Admin/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Boarding/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Concerns/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Dashboard/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Destination/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Destination/New/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Dev/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Fleet/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/ImageScan/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Notifications/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Profile/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Application/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Application/Backup/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Application/Preview/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Backup/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Clickhouse/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Dragonfly/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Keydb/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Mariadb/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Mongodb/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Mysql/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Postgresql/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Redis/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Database/Sqlite/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/New/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Resource/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Service/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Service/VolumeBackup/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Shared/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Shared/EnvironmentVariable/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Shared/ScheduledTask/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Project/Shared/Storages/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Security/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Security/CloudInitScript/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Security/CloudProviderToken/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Security/PrivateKey/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/Analytics/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/CaCertificate/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/CloudProviderToken/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/DockerRegistries/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/New/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/PrivateKey/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/Proxy/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/Security/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Server/Sentinel/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Settings/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/SharedVariables/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/SharedVariables/Environment/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/SharedVariables/Project/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/SharedVariables/Server/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/SharedVariables/Team/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Source/Gitea/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Source/Github/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Source/Gitlab/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Storage/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Subscription/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Tags/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Team/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Team/Member/`: `laravel/framework` has no Rust equivalent.
- `app/Livewire/Terminal/`: `laravel/framework` has no Rust equivalent.
- `app/Logging/`: `laravel/framework` has no Rust equivalent.
- `app/Mcp/Concerns/`: `laravel/framework` has no Rust equivalent.
- `app/Mcp/Tools/`: `laravel/framework` has no Rust equivalent.
- `app/Models/`: `laravel/framework` has no Rust equivalent.
- `app/Models/Scopes/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/Application/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/Channels/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/Container/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/Database/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/ImageScan/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/Internal/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/ScheduledTask/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/Server/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/TransactionalEmails/`: `laravel/framework` has no Rust equivalent.
- `app/Notifications/VolumeBackup/`: `laravel/framework` has no Rust equivalent.
- `app/Policies/`: `laravel/framework` has no Rust equivalent.
- `app/Providers/`: `laravel/framework` has no Rust equivalent.
- `app/Repositories/`: `laravel/framework` has no Rust equivalent.
- `app/Rules/`: `laravel/framework` has no Rust equivalent.
- `app/Services/`: `laravel/framework` has no Rust equivalent.
- `app/Services/Access/`: `laravel/framework` has no Rust equivalent.
- `app/Services/Auth/`: `laravel/framework` has no Rust equivalent.
- `app/Services/DeploymentConfiguration/`: `laravel/framework` has no Rust equivalent.
- `app/Services/Dns/`: `laravel/framework` has no Rust equivalent.
- `app/Services/GithubRunner/`: `laravel/framework` has no Rust equivalent.
- `app/Services/ImageScan/`: `laravel/framework` has no Rust equivalent.
- `app/Services/SecretManagers/`: `laravel/framework` has no Rust equivalent.
- `app/Services/Security/`: `laravel/framework` has no Rust equivalent.
- `app/Services/ServerTransfer/`: `laravel/framework` has no Rust equivalent.
- `app/Support/`: `laravel/framework` has no Rust equivalent.
- `app/Support/Actions/`: `laravel/framework` has no Rust equivalent.
- `app/Support/DatabaseImport/`: `laravel/framework` has no Rust equivalent.
- `app/Traits/`: `laravel/framework` has no Rust equivalent.
- `app/View/Components/Forms/`: `laravel/framework` has no Rust equivalent.
- `app/View/Components/Services/`: `laravel/framework` has no Rust equivalent.
- `bootstrap/`: Uses dynamic code (eval, new Function or computed imports).
- `bootstrap/helpers/`: `laravel/framework` has no Rust equivalent.
- `config/`: `laravel/framework` has no Rust equivalent.
- `database/factories/`: `laravel/framework` has no Rust equivalent.
- `database/migrations/`: `laravel/framework` has no Rust equivalent.
- `database/seeders/`: `laravel/framework` has no Rust equivalent.
- `public/`: `laravel/framework` has no Rust equivalent.
- `routes/`: `laravel/framework` has no Rust equivalent.
- `scripts/`: `laravel/framework` has no Rust equivalent.
- `scripts/transfer-test/`: `laravel/framework` has no Rust equivalent.

## Risk guide

- **String offsets and slicing** (High): JS indexes strings in UTF-16 code units, Rust in UTF-8 bytes. Offsets that cross the boundary must be converted, or non-ASCII text gets wrong ranges silently.
- **Regex features the `regex` crate lacks** (High): Lookaround, backreferences and the `y` flag are not in the `regex` crate. Use `fancy-regex` for those patterns, or rewrite them, and test them on non-ASCII text: `\w`, `\b` and case-insensitive matching differ between engines even where a pattern compiles. JS `/i` without `u` never matches a non-ASCII letter to an ASCII one, Rust `(?i)` does (`ſ` matches `s`, the Kelvin sign `K` matches `k`): write `(?i-u)` or compare with `eq_ignore_ascii_case`.
- **Regexes built at runtime** (High): Patterns come from data or config in JS syntax. Rust regex syntax differs, so user-visible behavior can change. Pin it with tests on real patterns.
- **Dynamic code (eval, new Function, computed import)** (High): eval, new Function and computed imports have no Rust equivalent. Keep this part in JS or replace it with an explicit registry.
- **Dynamic typing** (Medium): Values whose shape is checked at runtime need an explicit Rust enum or `serde_json::Value`.
- **Insertion-ordered maps and object keys** (Medium): JS Map, Set and object keys keep insertion order. `HashMap` does not: use `indexmap` wherever order reaches output. With serde_json's `preserve_order`, `Map::remove` swaps the last key into the hole: use `shift_remove` for JS `delete`. Sort with `js::sort` when a comparator can return NaN: the order then depends on V8's algorithm.
- **Exceptions** (Low): Thrown errors become `Result`. Errors crossing the bridge must map back to the same JS error shape.
- **Generators** (Medium): Lazy generators become iterators, or return a collected Vec across the bridge.
- **Class inheritance** (Medium): Class hierarchies become traits plus composition; `instanceof` checks become enums.
- **Reflection and prototypes** (Medium): Proxy, Reflect, Symbol and prototype tricks need a redesign, not a translation.
- **Callback-shaped APIs** (Medium): Function-typed parameters and methods cross the bridge on every call. Keep boundaries coarse: pass data in, get data out.
- **Number semantics** (Low): JS numbers are f64; bitwise ops truncate to i32; parseInt is lenient. Pick Rust integer types deliberately. Printing differs: `${n}` gives `1e+21`, `1e-7` and `0` for -0 where Rust's `{}` does not, so use `js::number_to_string`. `Math.round` rounds halves up (-2.5 gives -2), Rust's `round` away from zero: use `js::math_round`. `toFixed` rounds exact halves up ((2.5).toFixed(0) is "3"), Rust's `{:.0}` to even.
- **JSON round-trips** (Low): JSON.stringify key order and number formatting must match `serde_json` output if the result is user-visible.
- **Integer width and overflow** (Medium): Python ints never overflow; C, Go and Java ints wrap or are undefined on overflow. Pick i64, i128 or a bignum crate deliberately, and use wrapping_* where the source wraps.
- **Division and modulo** (Medium): Python `//` and `%` floor toward negative infinity; Rust `/` and `%` truncate: use `div_euclid`/`rem_euclid` or explicit floor math. C division by zero is undefined.
- **Concurrency** (Medium): Threads, goroutines and locks need a design in Rust (channels, Send/Sync), not a line-by-line translation; keep concurrency in the caller where possible.
- **Null references** (Medium): Any reference may be null in the source: decide per parameter whether it is `Option` and what null means, instead of mapping every value to `Option`.

## Whole app

Shape: **server**.

| Entry point | Kind | Found from |
| --- | --- | --- |
| `public/index.php` | server | `public/index.php`, a front controller: serve it with `php -S` |
| `app/Actions/Development/StartDevelopmentQemuVm.php` | cli | `$argv` at app/Actions/Development/StartDevelopmentQemuVm.php:390 |
| `app/Http/Controllers/Api/DigitalOceanController.php` | server | `createServer(` at app/Http/Controllers/Api/DigitalOceanController.php:217 |
| `app/Http/Controllers/Api/HetznerController.php` | server | `createServer(` at app/Http/Controllers/Api/HetznerController.php:773 |
| `app/Http/Controllers/Api/HostingerController.php` | server | `createServer(` at app/Http/Controllers/Api/HostingerController.php:133 |
| `app/Http/Controllers/Api/VultrController.php` | server | `createServer(` at app/Http/Controllers/Api/VultrController.php:200 |
| `app/Livewire/Server/New/ByHetzner.php` | server | `createServer(` at app/Livewire/Server/New/ByHetzner.php:677 |
| `app/Services/HetznerService.php` | server | `createServer(` at app/Services/HetznerService.php:173 |
| `scripts/transfer-test/snapshot-diff.php` | cli | `$argv` at scripts/transfer-test/snapshot-diff.php:13 |

### Every dependency

| Dependency | Declared | Used by | Rust | Fit | Note |
| --- | --- | --- | --- | --- | --- |
| `laravel/framework` | yes | 144 | axum | none | a web framework is an application rewrite, not a port |
| `Livewire` | no | 69 | ? | unknown |  |
| `Laravel` | no | 29 | ? | unknown |  |
| `Spatie` | no | 23 | ? | unknown |  |
| `Lorisleiva` | no | 12 | ? | unknown |  |
| `Stripe` | no | 7 | ? | unknown |  |
| `symfony/http-kernel` | no | 7 | ? | unknown |  |
| `Cron` | no | 5 | ? | unknown |  |
| `phpseclib3` | no | 4 | ? | unknown |  |
| `DanHarrin` | no | 3 | ? | unknown |  |
| `SocialiteProviders` | no | 3 | ? | unknown |  |
| `Aws` | no | 2 | ? | unknown |  |
| `OpenApi` | no | 2 | ? | unknown |  |
| `Pion` | no | 2 | ? | unknown |  |
| `Stevebauman` | no | 2 | ? | unknown |  |
| `symfony/mailer` | no | 2 | ? | unknown |  |
| `symfony/mime` | no | 2 | ? | unknown |  |
| `Lcobucci` | no | 1 | ? | unknown |  |
| `Nubs` | no | 1 | ? | unknown |  |
| `Pdo` | no | 1 | ? | unknown |  |
| `Poliander` | no | 1 | ? | unknown |  |
| `Rector` | no | 1 | ? | unknown |  |
| `RectorLaravel` | no | 1 | ? | unknown |  |
| `Resend` | no | 1 | ? | unknown |  |
| `Termwind` | no | 1 | ? | unknown |  |
| `Visus` | no | 1 | ? | unknown |  |
| `danharrin/livewire-rate-limiting` | yes | 0 | ? | unknown |  |
| `laravel/fortify` | yes | 0 | ? | unknown |  |
| `laravel/horizon` | yes | 0 | ? | unknown |  |
| `laravel/mcp` | yes | 0 | ? | unknown |  |
| `laravel/pail` | yes | 0 | ? | unknown |  |
| `laravel/prompts` | yes | 0 | ? | unknown |  |
| `laravel/reverb` | yes | 0 | ? | unknown |  |
| `laravel/sanctum` | yes | 0 | ? | unknown |  |
| `laravel/socialite` | yes | 0 | ? | unknown |  |
| `laravel/tinker` | yes | 0 | ? | unknown |  |
| `laravel/ui` | yes | 0 | ? | unknown |  |
| `league/flysystem-aws-s3-v3` | yes | 0 | ? | unknown |  |
| `league/flysystem-sftp-v3` | yes | 0 | ? | unknown |  |
| `livewire/livewire` | yes | 0 | ? | unknown |  |
| `log1x/laravel-webfonts` | yes | 0 | ? | unknown |  |
| `lorisleiva/laravel-actions` | yes | 0 | ? | unknown |  |
| `nubs/random-name-generator` | yes | 0 | ? | unknown |  |
| `phpseclib/phpseclib` | yes | 0 | ? | unknown |  |
| `pion/laravel-chunk-upload` | yes | 0 | ? | unknown |  |
| `poliander/cron` | yes | 0 | ? | unknown |  |
| `pusher/pusher-php-server` | yes | 0 | ? | unknown |  |
| `resend/resend-laravel` | yes | 0 | ? | unknown |  |
| `socialiteproviders/authentik` | yes | 0 | ? | unknown |  |
| `socialiteproviders/clerk` | yes | 0 | ? | unknown |  |
| `socialiteproviders/discord` | yes | 0 | ? | unknown |  |
| `socialiteproviders/google` | yes | 0 | ? | unknown |  |
| `socialiteproviders/infomaniak` | yes | 0 | ? | unknown |  |
| `socialiteproviders/microsoft-azure` | yes | 0 | ? | unknown |  |
| `socialiteproviders/zitadel` | yes | 0 | ? | unknown |  |
| `spatie/laravel-activitylog` | yes | 0 | ? | unknown |  |
| `spatie/laravel-data` | yes | 0 | ? | unknown |  |
| `spatie/laravel-markdown` | yes | 0 | ? | unknown |  |
| `spatie/laravel-schemaless-attributes` | yes | 0 | ? | unknown |  |
| `spatie/url` | yes | 0 | ? | unknown |  |
| `stevebauman/purify` | yes | 0 | ? | unknown |  |
| `stripe/stripe-php` | yes | 0 | ? | unknown |  |
| `visus/cuid2` | yes | 0 | ? | unknown |  |
| `yosymfony/toml` | yes | 0 | ? | unknown |  |
| `zircote/swagger-php` | yes | 0 | ? | unknown |  |
| `psr/http-message` (also `symfony/http-foundation`) | no | 11 | http, axum | partial |  |
| `league/flysystem` | no | 3 | object_store, opendal | partial |  |
| `nesbot/carbon` | no | 17 | chrono, jiff | good |  |
| `symfony/yaml` | yes | 16 | serde_yaml | good |  |
| `guzzlehttp/guzzle` | yes | 6 | reqwest | good |  |
| `monolog/monolog` (also `psr/log`) | no | 3 | tracing, log | good |  |
| `symfony/process` | no | 2 | std::process | good |  |
| `firebase/php-jwt` (also `lcobucci/jwt`) | yes | 1 | jsonwebtoken | good |  |
| `symfony/finder` | no | 1 | walkdir, globset | good |  |
| `doctrine/dbal` | yes | 0 | sqlx | good |  |

### Pieces, in port order

- wave 0: `app/Auth/` (22 lines)
- wave 0: `app/Auth/Oidc/Exceptions/` (16 lines)
- wave 0: `app/Casts/` (43 lines)
- wave 0: `app/Console/Commands/Generate/` (262 lines)
- wave 0: `app/Contracts/` (30 lines)
- wave 0: `app/Data/Traffic/` (131 lines)
- wave 0: `app/Livewire/Dev/` (25 lines)
- wave 0: `app/Livewire/Notifications/Concerns/` (40 lines)
- wave 0: `app/Mcp/Prompts/` (108 lines)
- wave 0: `app/Notifications/Dto/` (136 lines)
- wave 0: `app/Services/DeploymentConfiguration/Concerns/` (26 lines)
- wave 0: `app/Services/Security/` (285 lines)
- wave 0: `app/Support/Actions/` (11 lines)
- wave 0: `app/Support/DomainConnect/` (296 lines)
- wave 0: `app/Support/Metrics/` (33 lines)
- wave 0: `app/View/Components/Forms/` (353 lines)
- wave 0: `bootstrap/` (52 lines)
- wave 0: `lang/en/` (18 lines)
- wave 0: `rector.php` (30 lines)
- wave 0: `resources/views/auth/` (407 lines)
- wave 0: `resources/views/components/` (5226 lines)
- wave 0: `resources/views/components/application/` (390 lines)
- wave 0: `resources/views/components/applications/` (121 lines)
- wave 0: `resources/views/components/auth/` (101 lines)
- wave 0: `resources/views/components/dashboard/` (146 lines)
- wave 0: `resources/views/components/database/` (114 lines)
- wave 0: `resources/views/components/deployment/` (110 lines)
- wave 0: `resources/views/components/emails/` (23 lines)
- wave 0: `resources/views/components/forms/` (1354 lines)
- wave 0: `resources/views/components/image-scan/` (239 lines)
- wave 0: `resources/views/components/notification/` (262 lines)
- wave 0: `resources/views/components/security/` (51 lines)
- wave 0: `resources/views/components/server/` (640 lines)
- wave 0: `resources/views/components/service-database/` (20 lines)
- wave 0: `resources/views/components/service/` (87 lines)
- wave 0: `resources/views/components/services/` (39 lines)
- wave 0: `resources/views/components/settings/` (33 lines)
- wave 0: `resources/views/components/shared-variables/` (214 lines)
- wave 0: `resources/views/components/skeleton/` (159 lines)
- wave 0: `resources/views/components/status/` (81 lines)
- wave 0: `resources/views/components/table/` (236 lines)
- wave 0: `resources/views/components/team/` (62 lines)
- wave 0: `resources/views/components/terminal/` (78 lines)
- wave 0: `resources/views/components/theme-controls/` (20 lines)
- wave 0: `resources/views/emails/` (403 lines)
- wave 0: `resources/views/errors/` (110 lines)
- wave 0: `resources/views/invitation/` (32 lines)
- wave 0: `resources/views/layouts/` (880 lines)
- wave 0: `resources/views/livewire/` (3706 lines)
- wave 0: `resources/views/livewire/admin/` (105 lines)
- wave 0: `resources/views/livewire/boarding/` (647 lines)
- wave 0: `resources/views/livewire/dashboard/` (411 lines)
- wave 0: `resources/views/livewire/destination/` (288 lines)
- wave 0: `resources/views/livewire/destination/new/` (28 lines)
- wave 0: `resources/views/livewire/dev/` (21 lines)
- wave 0: `resources/views/livewire/fleet/` (471 lines)
- wave 0: `resources/views/livewire/image-scan/` (902 lines)
- wave 0: `resources/views/livewire/image-scan/partials/` (469 lines)
- wave 0: `resources/views/livewire/notifications/` (335 lines)
- wave 0: `resources/views/livewire/partials/` (163 lines)
- wave 0: `resources/views/livewire/profile/` (447 lines)
- wave 0: `resources/views/livewire/project/` (1057 lines)
- wave 0: `resources/views/livewire/project/application/` (3031 lines)
- wave 0: `resources/views/livewire/project/application/backup/` (272 lines)
- wave 0: `resources/views/livewire/project/application/deployment/` (903 lines)
- wave 0: `resources/views/livewire/project/application/partials/` (177 lines)
- wave 0: `resources/views/livewire/project/application/preview/` (25 lines)
- wave 0: `resources/views/livewire/project/database/` (1015 lines)
- wave 0: `resources/views/livewire/project/database/backup-edit/` (200 lines)
- wave 0: `resources/views/livewire/project/database/backup/` (202 lines)
- wave 0: `resources/views/livewire/project/database/clickhouse/` (121 lines)
- wave 0: `resources/views/livewire/project/database/dragonfly/` (100 lines)
- wave 0: `resources/views/livewire/project/database/keydb/` (111 lines)
- wave 0: `resources/views/livewire/project/database/mariadb/` (114 lines)
- wave 0: `resources/views/livewire/project/database/mongodb/` (111 lines)
- wave 0: `resources/views/livewire/project/database/mysql/` (114 lines)
- wave 0: `resources/views/livewire/project/database/postgresql/` (149 lines)
- wave 0: `resources/views/livewire/project/database/redis/` (120 lines)
- wave 0: `resources/views/livewire/project/database/sqlite/` (118 lines)
- wave 0: `resources/views/livewire/project/new/` (1747 lines)
- wave 0: `resources/views/livewire/project/new/partials/` (97 lines)
- wave 0: `resources/views/livewire/project/partials/` (10 lines)
- wave 0: `resources/views/livewire/project/resource/` (586 lines)
- wave 0: `resources/views/livewire/project/service/` (2307 lines)
- wave 0: `resources/views/livewire/project/service/partials/` (214 lines)
- wave 0: `resources/views/livewire/project/service/volume-backup/` (394 lines)
- wave 0: `resources/views/livewire/project/shared/` (3257 lines)
- wave 0: `resources/views/livewire/project/shared/environment-variable/` (723 lines)
- wave 0: `resources/views/livewire/project/shared/partials/` (8 lines)
- wave 0: `resources/views/livewire/project/shared/scheduled-task/` (287 lines)
- wave 0: `resources/views/livewire/project/shared/storages/` (430 lines)
- wave 0: `resources/views/livewire/project/shared/storages/volume-backups/` (350 lines)
- wave 0: `resources/views/livewire/security/` (869 lines)
- wave 0: `resources/views/livewire/security/cloud-init-script/` (50 lines)
- wave 0: `resources/views/livewire/security/cloud-provider-token/` (60 lines)
- wave 0: `resources/views/livewire/security/partials/` (157 lines)
- wave 0: `resources/views/livewire/security/private-key/` (357 lines)
- wave 0: `resources/views/livewire/security/private-key/partials/` (79 lines)
- wave 0: `resources/views/livewire/server/` (3965 lines)
- wave 0: `resources/views/livewire/server/analytics/` (18 lines)
- wave 0: `resources/views/livewire/server/ca-certificate/` (82 lines)
- wave 0: `resources/views/livewire/server/cloud-provider-token/` (72 lines)
- wave 0: `resources/views/livewire/server/docker-registries/` (212 lines)
- wave 0: `resources/views/livewire/server/new/` (762 lines)
- wave 0: `resources/views/livewire/server/new/partials/` (9 lines)
- wave 0: `resources/views/livewire/server/partials/` (126 lines)
- wave 0: `resources/views/livewire/server/private-key/` (119 lines)
- wave 0: `resources/views/livewire/server/proxy/` (376 lines)
- wave 0: `resources/views/livewire/server/security/` (910 lines)
- wave 0: `resources/views/livewire/server/sentinel/` (62 lines)
- wave 0: `resources/views/livewire/settings/` (380 lines)
- wave 0: `resources/views/livewire/shared-variables/` (141 lines)
- wave 0: `resources/views/livewire/shared-variables/environment/` (68 lines)
- wave 0: `resources/views/livewire/shared-variables/project/` (71 lines)
- wave 0: `resources/views/livewire/shared-variables/server/` (53 lines)
- wave 0: `resources/views/livewire/shared-variables/team/` (7 lines)
- wave 0: `resources/views/livewire/shared/` (13 lines)
- wave 0: `resources/views/livewire/source/gitea/` (360 lines)
- wave 0: `resources/views/livewire/source/github/` (665 lines)
- wave 0: `resources/views/livewire/source/gitlab/` (355 lines)
- wave 0: `resources/views/livewire/subscription/` (420 lines)
- wave 0: `resources/views/livewire/tags/` (466 lines)
- wave 0: `resources/views/livewire/tags/partials/` (7 lines)
- wave 0: `resources/views/livewire/team/` (707 lines)
- wave 0: `resources/views/livewire/team/member/` (336 lines)
- wave 0: `resources/views/livewire/terminal/` (343 lines)
- wave 0: `resources/views/livewire/traffic/` (896 lines)
- wave 0: `resources/views/proxy/` (124 lines)
- wave 0: `resources/views/server/` (9 lines)
- wave 0: `resources/views/source/` (193 lines)
- wave 0: `resources/views/source/github/` (4 lines)
- wave 1: `app/Logging/` (79 lines)
- wave 1: `app/Notifications/Internal/` (60 lines)
- wave 1: `app/Repositories/` (40 lines)
- wave 2: `app/Actions/Application/`, `app/Actions/Database/`, `app/Actions/Docker/`, `app/Actions/Fortify/`, `app/Actions/ImageScan/`, `app/Actions/Proxy/`, `app/Actions/Server/`, `app/Actions/Service/`, `app/Actions/Shared/`, `app/Actions/Stripe/`, `app/Actions/Team/`, `app/Actions/TinyHelmTask/`, `app/Actions/User/`, `app/Auth/Oidc/`, `app/Auth/Oidc/Socialite/`, `app/Data/`, `app/Enums/`, `app/Events/`, `app/Exceptions/`, `app/Helpers/`, `app/Http/Controllers/Webhook/Concerns/`, `app/Http/Middleware/`, `app/Jobs/`, `app/Livewire/`, `app/Livewire/Concerns/`, `app/Livewire/Server/`, `app/Models/`, `app/Models/Scopes/`, `app/Notifications/`, `app/Notifications/Application/`, `app/Notifications/Channels/`, `app/Notifications/Container/`, `app/Notifications/Database/`, `app/Notifications/ImageScan/`, `app/Notifications/ScheduledTask/`, `app/Notifications/Server/`, `app/Notifications/TransactionalEmails/`, `app/Notifications/VolumeBackup/`, `app/Policies/`, `app/Providers/`, `app/Rules/`, `app/Services/`, `app/Services/Access/`, `app/Services/DeploymentConfiguration/`, `app/Services/Dns/`, `app/Services/GithubRunner/`, `app/Services/ImageScan/`, `app/Services/ServerTransfer/`, `app/Support/`, `app/Support/DatabaseImport/`, `app/Traits/`, tangled: one piece (116709 lines)
- wave 3: `app/Actions/Destination/` (12 lines)
- wave 3: `app/Actions/Development/` (725 lines)
- wave 3: `app/Actions/IntegrationToken/` (228 lines)
- wave 3: `app/Console/` (183 lines)
- wave 3: `app/Console/Commands/Cloud/` (1505 lines)
- wave 3: `app/Http/` (123 lines)
- wave 3: `app/Listeners/` (376 lines)
- wave 3: `app/Livewire/Admin/` (88 lines)
- wave 3: `app/Livewire/Boarding/` (439 lines)
- wave 3: `app/Livewire/Dashboard/` (285 lines)
- wave 3: `app/Livewire/Destination/` (243 lines)
- wave 3: `app/Livewire/Destination/New/` (97 lines)
- wave 3: `app/Livewire/Fleet/` (359 lines)
- wave 3: `app/Livewire/Forms/` (93 lines)
- wave 3: `app/Livewire/ImageScan/` (2053 lines)
- wave 3: `app/Livewire/Notifications/` (1581 lines)
- wave 3: `app/Livewire/Profile/` (380 lines)
- wave 3: `app/Livewire/Project/` (834 lines)
- wave 3: `app/Livewire/Project/Application/Backup/` (255 lines)
- wave 3: `app/Livewire/Project/Application/Deployment/` (586 lines)
- wave 3: `app/Livewire/Project/Application/Preview/` (59 lines)
- wave 3: `app/Livewire/Project/Database/` (1855 lines)
- wave 3: `app/Livewire/Project/Database/Backup/` (242 lines)
- wave 3: `app/Livewire/Project/Database/Clickhouse/` (247 lines)
- wave 3: `app/Livewire/Project/Database/Dragonfly/` (219 lines)
- wave 3: `app/Livewire/Project/Database/Keydb/` (223 lines)
- wave 3: `app/Livewire/Project/Database/Mariadb/` (240 lines)
- wave 3: `app/Livewire/Project/Database/Mongodb/` (260 lines)
- wave 3: `app/Livewire/Project/Database/Mysql/` (265 lines)
- wave 3: `app/Livewire/Project/Database/Postgresql/` (400 lines)
- wave 3: `app/Livewire/Project/Database/Sqlite/` (346 lines)
- wave 3: `app/Livewire/Project/New/` (1960 lines)
- wave 3: `app/Livewire/Project/Resource/` (380 lines)
- wave 3: `app/Livewire/Project/Service/VolumeBackup/` (330 lines)
- wave 3: `app/Livewire/Project/Shared/EnvironmentVariable/` (1861 lines)
- wave 3: `app/Livewire/Project/Shared/ScheduledTask/` (550 lines)
- wave 3: `app/Livewire/Security/CloudInitScript/` (84 lines)
- wave 3: `app/Livewire/Security/CloudProviderToken/` (167 lines)
- wave 3: `app/Livewire/Security/PrivateKey/` (483 lines)
- wave 3: `app/Livewire/Server/Analytics/` (20 lines)
- wave 3: `app/Livewire/Server/CaCertificate/` (120 lines)
- wave 3: `app/Livewire/Server/CloudProviderToken/` (197 lines)
- wave 3: `app/Livewire/Server/DockerRegistries/` (264 lines)
- wave 3: `app/Livewire/Server/New/Concerns/` (26 lines)
- wave 3: `app/Livewire/Server/PrivateKey/` (115 lines)
- wave 3: `app/Livewire/Server/Proxy/` (500 lines)
- wave 3: `app/Livewire/Server/Sentinel/` (54 lines)
- wave 3: `app/Livewire/Settings/` (705 lines)
- wave 3: `app/Livewire/SharedVariables/` (142 lines)
- wave 3: `app/Livewire/SharedVariables/Environment/` (182 lines)
- wave 3: `app/Livewire/SharedVariables/Project/` (173 lines)
- wave 3: `app/Livewire/SharedVariables/Server/` (183 lines)
- wave 3: `app/Livewire/SharedVariables/Team/` (154 lines)
- wave 3: `app/Livewire/Source/Gitea/` (390 lines)
- wave 3: `app/Livewire/Source/Github/` (512 lines)
- wave 3: `app/Livewire/Source/Gitlab/` (376 lines)
- wave 3: `app/Livewire/Storage/Concerns/` (63 lines)
- wave 3: `app/Livewire/Subscription/` (451 lines)
- wave 3: `app/Livewire/Team/Member/` (312 lines)
- wave 3: `app/Livewire/Terminal/` (81 lines)
- wave 3: `app/Mcp/Concerns/`, `app/Mcp/Resources/`, `app/Mcp/Servers/`, `app/Mcp/Tools/`, tangled: one piece (7777 lines)
- wave 3: `app/Services/Auth/` (439 lines)
- wave 3: `app/Services/SecretManagers/` (162 lines)
- wave 3: `app/View/Components/Services/` (50 lines)
- wave 3: `bootstrap/helpers/` (18126 lines)
- wave 3: `config/` (2596 lines)
- wave 3: `database/factories/` (495 lines)
- wave 3: `database/seeders/` (1161 lines)
- wave 3: `scripts/transfer-test/` (1049 lines)
- wave 4: `app/Console/Commands/` (5162 lines)
- wave 4: `app/Http/Controllers/` (540 lines)
- wave 4: `app/Livewire/Project/Database/Redis/` (242 lines)
- wave 4: `app/Livewire/Server/New/` (2106 lines)
- wave 4: `app/Livewire/Server/Security/` (829 lines)
- wave 4: `app/Livewire/Storage/` (742 lines)
- wave 4: `app/Livewire/Team/` (770 lines)
- wave 4: `database/migrations/` (17276 lines)
- wave 4: `public/` (44 lines)
- wave 4: `scripts/` (257 lines)
- wave 5: `app/Http/Controllers/Api/`, `app/Http/Controllers/Api/Concerns/`, tangled: one piece (34331 lines)
- wave 5: `app/Http/Controllers/Webhook/` (2527 lines)
- wave 5: `app/Livewire/Project/Shared/` (3711 lines)
- wave 5: `app/Livewire/Security/` (1092 lines)
- wave 6: `app/Livewire/Project/Service/`, `app/Livewire/Project/Shared/Storages/`, tangled: one piece (4803 lines)
- wave 6: `app/Livewire/Tags/` (254 lines)
- wave 7: `app/Livewire/Project/Application/` (5418 lines)
- wave 8: `routes/` (1179 lines)

### Decisions for the user

**D1** (shape): The app is a server, entered through public/index.php, app/Actions/Development/StartDevelopmentQemuVm.php, app/Http/Controllers/Api/DigitalOceanController.php, app/Http/Controllers/Api/HetznerController.php, app/Http/Controllers/Api/HostingerController.php, app/Http/Controllers/Api/VultrController.php, app/Livewire/Server/New/ByHetzner.php, app/Services/HetznerService.php, scripts/transfer-test/snapshot-diff.php. Port every entry point, in that shape?

> Yes: an axum front door (`app front-door`) takes the server over first

**D2** (blocked app/Actions/Application/): app/Actions/Application/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D3** (blocked app/Actions/Database/): app/Actions/Database/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D4** (blocked app/Actions/Development/): app/Actions/Development/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D5** (blocked app/Actions/Docker/): app/Actions/Docker/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D6** (blocked app/Actions/Fortify/): app/Actions/Fortify/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D7** (blocked app/Actions/ImageScan/): app/Actions/ImageScan/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D8** (blocked app/Actions/IntegrationToken/): app/Actions/IntegrationToken/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D9** (blocked app/Actions/Proxy/): app/Actions/Proxy/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D10** (blocked app/Actions/Server/): app/Actions/Server/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D11** (blocked app/Actions/Service/): app/Actions/Service/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D12** (blocked app/Actions/Shared/): app/Actions/Shared/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D13** (blocked app/Actions/Stripe/): app/Actions/Stripe/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D14** (blocked app/Actions/Team/): app/Actions/Team/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D15** (blocked app/Actions/TinyHelmTask/): app/Actions/TinyHelmTask/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D16** (blocked app/Actions/User/): app/Actions/User/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D17** (blocked app/Auth/Oidc/): app/Auth/Oidc/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D18** (blocked app/Auth/Oidc/Socialite/): app/Auth/Oidc/Socialite/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D19** (blocked app/Casts/): app/Casts/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D20** (blocked app/Console/): app/Console/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D21** (blocked app/Console/Commands/): app/Console/Commands/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D22** (blocked app/Console/Commands/Cloud/): app/Console/Commands/Cloud/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D23** (blocked app/Console/Commands/Generate/): app/Console/Commands/Generate/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D24** (blocked app/Contracts/): app/Contracts/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D25** (blocked app/Events/): app/Events/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D26** (blocked app/Exceptions/): app/Exceptions/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D27** (blocked app/Helpers/): app/Helpers/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D28** (blocked app/Http/): app/Http/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D29** (blocked app/Http/Controllers/): app/Http/Controllers/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D30** (blocked app/Http/Controllers/Api/): app/Http/Controllers/Api/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D31** (blocked app/Http/Controllers/Api/Concerns/): app/Http/Controllers/Api/Concerns/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D32** (blocked app/Http/Controllers/Webhook/): app/Http/Controllers/Webhook/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D33** (blocked app/Http/Controllers/Webhook/Concerns/): app/Http/Controllers/Webhook/Concerns/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D34** (blocked app/Http/Middleware/): app/Http/Middleware/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D35** (blocked app/Jobs/): app/Jobs/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D36** (blocked app/Listeners/): app/Listeners/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D37** (blocked app/Livewire/): app/Livewire/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D38** (blocked app/Livewire/Admin/): app/Livewire/Admin/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D39** (blocked app/Livewire/Boarding/): app/Livewire/Boarding/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D40** (blocked app/Livewire/Concerns/): app/Livewire/Concerns/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D41** (blocked app/Livewire/Dashboard/): app/Livewire/Dashboard/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D42** (blocked app/Livewire/Destination/): app/Livewire/Destination/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D43** (blocked app/Livewire/Destination/New/): app/Livewire/Destination/New/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D44** (blocked app/Livewire/Dev/): app/Livewire/Dev/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D45** (blocked app/Livewire/Fleet/): app/Livewire/Fleet/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D46** (blocked app/Livewire/ImageScan/): app/Livewire/ImageScan/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D47** (blocked app/Livewire/Notifications/): app/Livewire/Notifications/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D48** (blocked app/Livewire/Profile/): app/Livewire/Profile/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D49** (blocked app/Livewire/Project/): app/Livewire/Project/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D50** (blocked app/Livewire/Project/Application/): app/Livewire/Project/Application/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D51** (blocked app/Livewire/Project/Application/Backup/): app/Livewire/Project/Application/Backup/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D52** (blocked app/Livewire/Project/Application/Preview/): app/Livewire/Project/Application/Preview/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D53** (blocked app/Livewire/Project/Database/): app/Livewire/Project/Database/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D54** (blocked app/Livewire/Project/Database/Backup/): app/Livewire/Project/Database/Backup/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D55** (blocked app/Livewire/Project/Database/Clickhouse/): app/Livewire/Project/Database/Clickhouse/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D56** (blocked app/Livewire/Project/Database/Dragonfly/): app/Livewire/Project/Database/Dragonfly/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D57** (blocked app/Livewire/Project/Database/Keydb/): app/Livewire/Project/Database/Keydb/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D58** (blocked app/Livewire/Project/Database/Mariadb/): app/Livewire/Project/Database/Mariadb/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D59** (blocked app/Livewire/Project/Database/Mongodb/): app/Livewire/Project/Database/Mongodb/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D60** (blocked app/Livewire/Project/Database/Mysql/): app/Livewire/Project/Database/Mysql/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D61** (blocked app/Livewire/Project/Database/Postgresql/): app/Livewire/Project/Database/Postgresql/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D62** (blocked app/Livewire/Project/Database/Redis/): app/Livewire/Project/Database/Redis/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D63** (blocked app/Livewire/Project/Database/Sqlite/): app/Livewire/Project/Database/Sqlite/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D64** (blocked app/Livewire/Project/New/): app/Livewire/Project/New/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D65** (blocked app/Livewire/Project/Resource/): app/Livewire/Project/Resource/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D66** (blocked app/Livewire/Project/Service/): app/Livewire/Project/Service/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D67** (blocked app/Livewire/Project/Service/VolumeBackup/): app/Livewire/Project/Service/VolumeBackup/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D68** (blocked app/Livewire/Project/Shared/): app/Livewire/Project/Shared/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D69** (blocked app/Livewire/Project/Shared/EnvironmentVariable/): app/Livewire/Project/Shared/EnvironmentVariable/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D70** (blocked app/Livewire/Project/Shared/ScheduledTask/): app/Livewire/Project/Shared/ScheduledTask/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D71** (blocked app/Livewire/Project/Shared/Storages/): app/Livewire/Project/Shared/Storages/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D72** (blocked app/Livewire/Security/): app/Livewire/Security/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D73** (blocked app/Livewire/Security/CloudInitScript/): app/Livewire/Security/CloudInitScript/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D74** (blocked app/Livewire/Security/CloudProviderToken/): app/Livewire/Security/CloudProviderToken/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D75** (blocked app/Livewire/Security/PrivateKey/): app/Livewire/Security/PrivateKey/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D76** (blocked app/Livewire/Server/): app/Livewire/Server/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D77** (blocked app/Livewire/Server/Analytics/): app/Livewire/Server/Analytics/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D78** (blocked app/Livewire/Server/CaCertificate/): app/Livewire/Server/CaCertificate/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D79** (blocked app/Livewire/Server/CloudProviderToken/): app/Livewire/Server/CloudProviderToken/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D80** (blocked app/Livewire/Server/DockerRegistries/): app/Livewire/Server/DockerRegistries/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D81** (blocked app/Livewire/Server/New/): app/Livewire/Server/New/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D82** (blocked app/Livewire/Server/PrivateKey/): app/Livewire/Server/PrivateKey/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D83** (blocked app/Livewire/Server/Proxy/): app/Livewire/Server/Proxy/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D84** (blocked app/Livewire/Server/Security/): app/Livewire/Server/Security/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D85** (blocked app/Livewire/Server/Sentinel/): app/Livewire/Server/Sentinel/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D86** (blocked app/Livewire/Settings/): app/Livewire/Settings/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D87** (blocked app/Livewire/SharedVariables/): app/Livewire/SharedVariables/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D88** (blocked app/Livewire/SharedVariables/Environment/): app/Livewire/SharedVariables/Environment/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D89** (blocked app/Livewire/SharedVariables/Project/): app/Livewire/SharedVariables/Project/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D90** (blocked app/Livewire/SharedVariables/Server/): app/Livewire/SharedVariables/Server/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D91** (blocked app/Livewire/SharedVariables/Team/): app/Livewire/SharedVariables/Team/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D92** (blocked app/Livewire/Source/Gitea/): app/Livewire/Source/Gitea/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D93** (blocked app/Livewire/Source/Github/): app/Livewire/Source/Github/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D94** (blocked app/Livewire/Source/Gitlab/): app/Livewire/Source/Gitlab/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D95** (blocked app/Livewire/Storage/): app/Livewire/Storage/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D96** (blocked app/Livewire/Subscription/): app/Livewire/Subscription/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D97** (blocked app/Livewire/Tags/): app/Livewire/Tags/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D98** (blocked app/Livewire/Team/): app/Livewire/Team/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D99** (blocked app/Livewire/Team/Member/): app/Livewire/Team/Member/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D100** (blocked app/Livewire/Terminal/): app/Livewire/Terminal/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D101** (blocked app/Logging/): app/Logging/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D102** (blocked app/Mcp/Concerns/): app/Mcp/Concerns/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D103** (blocked app/Mcp/Tools/): app/Mcp/Tools/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D104** (blocked app/Models/): app/Models/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D105** (blocked app/Models/Scopes/): app/Models/Scopes/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D106** (blocked app/Notifications/): app/Notifications/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D107** (blocked app/Notifications/Application/): app/Notifications/Application/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D108** (blocked app/Notifications/Channels/): app/Notifications/Channels/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D109** (blocked app/Notifications/Container/): app/Notifications/Container/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D110** (blocked app/Notifications/Database/): app/Notifications/Database/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D111** (blocked app/Notifications/ImageScan/): app/Notifications/ImageScan/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D112** (blocked app/Notifications/Internal/): app/Notifications/Internal/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D113** (blocked app/Notifications/ScheduledTask/): app/Notifications/ScheduledTask/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D114** (blocked app/Notifications/Server/): app/Notifications/Server/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D115** (blocked app/Notifications/TransactionalEmails/): app/Notifications/TransactionalEmails/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D116** (blocked app/Notifications/VolumeBackup/): app/Notifications/VolumeBackup/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D117** (blocked app/Policies/): app/Policies/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D118** (blocked app/Providers/): app/Providers/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D119** (blocked app/Repositories/): app/Repositories/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D120** (blocked app/Rules/): app/Rules/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D121** (blocked app/Services/): app/Services/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D122** (blocked app/Services/Access/): app/Services/Access/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D123** (blocked app/Services/Auth/): app/Services/Auth/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D124** (blocked app/Services/DeploymentConfiguration/): app/Services/DeploymentConfiguration/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D125** (blocked app/Services/Dns/): app/Services/Dns/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D126** (blocked app/Services/GithubRunner/): app/Services/GithubRunner/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D127** (blocked app/Services/ImageScan/): app/Services/ImageScan/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D128** (blocked app/Services/SecretManagers/): app/Services/SecretManagers/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D129** (blocked app/Services/Security/): app/Services/Security/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D130** (blocked app/Services/ServerTransfer/): app/Services/ServerTransfer/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D131** (blocked app/Support/): app/Support/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D132** (blocked app/Support/Actions/): app/Support/Actions/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D133** (blocked app/Support/DatabaseImport/): app/Support/DatabaseImport/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D134** (blocked app/Traits/): app/Traits/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D135** (blocked app/View/Components/Forms/): app/View/Components/Forms/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D136** (blocked app/View/Components/Services/): app/View/Components/Services/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D137** (blocked bootstrap/): bootstrap/ is blocked: Uses dynamic code (eval, new Function or computed imports). Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D138** (blocked bootstrap/helpers/): bootstrap/helpers/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D139** (blocked config/): config/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D140** (blocked database/factories/): database/factories/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D141** (blocked database/migrations/): database/migrations/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D142** (blocked database/seeders/): database/seeders/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D143** (blocked public/): public/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D144** (blocked routes/): routes/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D145** (blocked scripts/): scripts/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D146** (blocked scripts/transfer-test/): scripts/transfer-test/ is blocked: `laravel/framework` has no Rust equivalent. Rework that part in Rust, or keep the unit outside?

> Research the blocker first; then port it with the piece that uses it

**D147** (dependency laravel/framework): No crate does what laravel/framework does (used by 144 units; a web framework is an application rewrite, not a port). Reimplement what the app uses, or drop the feature?

> Reimplement only the part the app uses, proved by its recorded cases

**D148** (dependency Livewire): Which crate replaces Livewire (used by 69 units)?

> Research it against primary sources into rust/research/Livewire.md before porting what uses it

**D149** (dependency Laravel): Which crate replaces Laravel (used by 29 units)?

> Research it against primary sources into rust/research/Laravel.md before porting what uses it

**D150** (dependency Spatie): Which crate replaces Spatie (used by 23 units)?

> Research it against primary sources into rust/research/Spatie.md before porting what uses it

**D151** (dependency Lorisleiva): Which crate replaces Lorisleiva (used by 12 units)?

> Research it against primary sources into rust/research/Lorisleiva.md before porting what uses it

**D152** (dependency Stripe): Which crate replaces Stripe (used by 7 units)?

> Research it against primary sources into rust/research/Stripe.md before porting what uses it

**D153** (dependency symfony/http-kernel): Which crate replaces symfony/http-kernel (used by 7 units)?

> Research it against primary sources into rust/research/symfony-http-kernel.md before porting what uses it

**D154** (dependency Cron): Which crate replaces Cron (used by 5 units)?

> Research it against primary sources into rust/research/Cron.md before porting what uses it

**D155** (dependency phpseclib3): Which crate replaces phpseclib3 (used by 4 units)?

> Research it against primary sources into rust/research/phpseclib3.md before porting what uses it

**D156** (dependency DanHarrin): Which crate replaces DanHarrin (used by 3 units)?

> Research it against primary sources into rust/research/DanHarrin.md before porting what uses it

**D157** (dependency SocialiteProviders): Which crate replaces SocialiteProviders (used by 3 units)?

> Research it against primary sources into rust/research/SocialiteProviders.md before porting what uses it

**D158** (dependency Aws): Which crate replaces Aws (used by 2 units)?

> Research it against primary sources into rust/research/Aws.md before porting what uses it

**D159** (dependency OpenApi): Which crate replaces OpenApi (used by 2 units)?

> Research it against primary sources into rust/research/OpenApi.md before porting what uses it

**D160** (dependency Pion): Which crate replaces Pion (used by 2 units)?

> Research it against primary sources into rust/research/Pion.md before porting what uses it

**D161** (dependency Stevebauman): Which crate replaces Stevebauman (used by 2 units)?

> Research it against primary sources into rust/research/Stevebauman.md before porting what uses it

**D162** (dependency symfony/mailer): Which crate replaces symfony/mailer (used by 2 units)?

> Research it against primary sources into rust/research/symfony-mailer.md before porting what uses it

**D163** (dependency symfony/mime): Which crate replaces symfony/mime (used by 2 units)?

> Research it against primary sources into rust/research/symfony-mime.md before porting what uses it

**D164** (dependency Lcobucci): Which crate replaces Lcobucci (used by 1 unit)?

> Research it against primary sources into rust/research/Lcobucci.md before porting what uses it

**D165** (dependency Nubs): Which crate replaces Nubs (used by 1 unit)?

> Research it against primary sources into rust/research/Nubs.md before porting what uses it

**D166** (dependency Pdo): Which crate replaces Pdo (used by 1 unit)?

> Research it against primary sources into rust/research/Pdo.md before porting what uses it

**D167** (dependency Poliander): Which crate replaces Poliander (used by 1 unit)?

> Research it against primary sources into rust/research/Poliander.md before porting what uses it

**D168** (dependency Rector): Which crate replaces Rector (used by 1 unit)?

> Research it against primary sources into rust/research/Rector.md before porting what uses it

**D169** (dependency RectorLaravel): Which crate replaces RectorLaravel (used by 1 unit)?

> Research it against primary sources into rust/research/RectorLaravel.md before porting what uses it

**D170** (dependency Resend): Which crate replaces Resend (used by 1 unit)?

> Research it against primary sources into rust/research/Resend.md before porting what uses it

**D171** (dependency Termwind): Which crate replaces Termwind (used by 1 unit)?

> Research it against primary sources into rust/research/Termwind.md before porting what uses it

**D172** (dependency Visus): Which crate replaces Visus (used by 1 unit)?

> Research it against primary sources into rust/research/Visus.md before porting what uses it

**D173** (dependency danharrin/livewire-rate-limiting): Which crate replaces danharrin/livewire-rate-limiting (declared, no unit imports it)?

> Research it against primary sources into rust/research/danharrin-livewire-rate-limiting.md before porting what uses it

**D174** (dependency laravel/fortify): Which crate replaces laravel/fortify (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-fortify.md before porting what uses it

**D175** (dependency laravel/horizon): Which crate replaces laravel/horizon (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-horizon.md before porting what uses it

**D176** (dependency laravel/mcp): Which crate replaces laravel/mcp (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-mcp.md before porting what uses it

**D177** (dependency laravel/pail): Which crate replaces laravel/pail (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-pail.md before porting what uses it

**D178** (dependency laravel/prompts): Which crate replaces laravel/prompts (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-prompts.md before porting what uses it

**D179** (dependency laravel/reverb): Which crate replaces laravel/reverb (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-reverb.md before porting what uses it

**D180** (dependency laravel/sanctum): Which crate replaces laravel/sanctum (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-sanctum.md before porting what uses it

**D181** (dependency laravel/socialite): Which crate replaces laravel/socialite (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-socialite.md before porting what uses it

**D182** (dependency laravel/tinker): Which crate replaces laravel/tinker (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-tinker.md before porting what uses it

**D183** (dependency laravel/ui): Which crate replaces laravel/ui (declared, no unit imports it)?

> Research it against primary sources into rust/research/laravel-ui.md before porting what uses it

**D184** (dependency league/flysystem-aws-s3-v3): Which crate replaces league/flysystem-aws-s3-v3 (declared, no unit imports it)?

> Research it against primary sources into rust/research/league-flysystem-aws-s3-v3.md before porting what uses it

**D185** (dependency league/flysystem-sftp-v3): Which crate replaces league/flysystem-sftp-v3 (declared, no unit imports it)?

> Research it against primary sources into rust/research/league-flysystem-sftp-v3.md before porting what uses it

**D186** (dependency livewire/livewire): Which crate replaces livewire/livewire (declared, no unit imports it)?

> Research it against primary sources into rust/research/livewire-livewire.md before porting what uses it

**D187** (dependency log1x/laravel-webfonts): Which crate replaces log1x/laravel-webfonts (declared, no unit imports it)?

> Research it against primary sources into rust/research/log1x-laravel-webfonts.md before porting what uses it

**D188** (dependency lorisleiva/laravel-actions): Which crate replaces lorisleiva/laravel-actions (declared, no unit imports it)?

> Research it against primary sources into rust/research/lorisleiva-laravel-actions.md before porting what uses it

**D189** (dependency nubs/random-name-generator): Which crate replaces nubs/random-name-generator (declared, no unit imports it)?

> Research it against primary sources into rust/research/nubs-random-name-generator.md before porting what uses it

**D190** (dependency phpseclib/phpseclib): Which crate replaces phpseclib/phpseclib (declared, no unit imports it)?

> Research it against primary sources into rust/research/phpseclib-phpseclib.md before porting what uses it

**D191** (dependency pion/laravel-chunk-upload): Which crate replaces pion/laravel-chunk-upload (declared, no unit imports it)?

> Research it against primary sources into rust/research/pion-laravel-chunk-upload.md before porting what uses it

**D192** (dependency poliander/cron): Which crate replaces poliander/cron (declared, no unit imports it)?

> Research it against primary sources into rust/research/poliander-cron.md before porting what uses it

**D193** (dependency pusher/pusher-php-server): Which crate replaces pusher/pusher-php-server (declared, no unit imports it)?

> Research it against primary sources into rust/research/pusher-pusher-php-server.md before porting what uses it

**D194** (dependency resend/resend-laravel): Which crate replaces resend/resend-laravel (declared, no unit imports it)?

> Research it against primary sources into rust/research/resend-resend-laravel.md before porting what uses it

**D195** (dependency socialiteproviders/authentik): Which crate replaces socialiteproviders/authentik (declared, no unit imports it)?

> Research it against primary sources into rust/research/socialiteproviders-authentik.md before porting what uses it

**D196** (dependency socialiteproviders/clerk): Which crate replaces socialiteproviders/clerk (declared, no unit imports it)?

> Research it against primary sources into rust/research/socialiteproviders-clerk.md before porting what uses it

**D197** (dependency socialiteproviders/discord): Which crate replaces socialiteproviders/discord (declared, no unit imports it)?

> Research it against primary sources into rust/research/socialiteproviders-discord.md before porting what uses it

**D198** (dependency socialiteproviders/google): Which crate replaces socialiteproviders/google (declared, no unit imports it)?

> Research it against primary sources into rust/research/socialiteproviders-google.md before porting what uses it

**D199** (dependency socialiteproviders/infomaniak): Which crate replaces socialiteproviders/infomaniak (declared, no unit imports it)?

> Research it against primary sources into rust/research/socialiteproviders-infomaniak.md before porting what uses it

**D200** (dependency socialiteproviders/microsoft-azure): Which crate replaces socialiteproviders/microsoft-azure (declared, no unit imports it)?

> Research it against primary sources into rust/research/socialiteproviders-microsoft-azure.md before porting what uses it

**D201** (dependency socialiteproviders/zitadel): Which crate replaces socialiteproviders/zitadel (declared, no unit imports it)?

> Research it against primary sources into rust/research/socialiteproviders-zitadel.md before porting what uses it

**D202** (dependency spatie/laravel-activitylog): Which crate replaces spatie/laravel-activitylog (declared, no unit imports it)?

> Research it against primary sources into rust/research/spatie-laravel-activitylog.md before porting what uses it

**D203** (dependency spatie/laravel-data): Which crate replaces spatie/laravel-data (declared, no unit imports it)?

> Research it against primary sources into rust/research/spatie-laravel-data.md before porting what uses it

**D204** (dependency spatie/laravel-markdown): Which crate replaces spatie/laravel-markdown (declared, no unit imports it)?

> Research it against primary sources into rust/research/spatie-laravel-markdown.md before porting what uses it

**D205** (dependency spatie/laravel-schemaless-attributes): Which crate replaces spatie/laravel-schemaless-attributes (declared, no unit imports it)?

> Research it against primary sources into rust/research/spatie-laravel-schemaless-attributes.md before porting what uses it

**D206** (dependency spatie/url): Which crate replaces spatie/url (declared, no unit imports it)?

> Research it against primary sources into rust/research/spatie-url.md before porting what uses it

**D207** (dependency stevebauman/purify): Which crate replaces stevebauman/purify (declared, no unit imports it)?

> Research it against primary sources into rust/research/stevebauman-purify.md before porting what uses it

**D208** (dependency stripe/stripe-php): Which crate replaces stripe/stripe-php (declared, no unit imports it)?

> Research it against primary sources into rust/research/stripe-stripe-php.md before porting what uses it

**D209** (dependency visus/cuid2): Which crate replaces visus/cuid2 (declared, no unit imports it)?

> Research it against primary sources into rust/research/visus-cuid2.md before porting what uses it

**D210** (dependency yosymfony/toml): Which crate replaces yosymfony/toml (declared, no unit imports it)?

> Research it against primary sources into rust/research/yosymfony-toml.md before porting what uses it

**D211** (dependency zircote/swagger-php): Which crate replaces zircote/swagger-php (declared, no unit imports it)?

> Research it against primary sources into rust/research/zircote-swagger-php.md before porting what uses it

**D212** (dependency psr/http-message): psr/http-message maps to http, axum only in part (used by 11 units). Use it?

> Use http, axum, and record cases for the behavior it does not cover

**D213** (dependency league/flysystem): league/flysystem maps to object_store, opendal only in part (used by 3 units). Use it?

> Use object_store, opendal, and record cases for the behavior it does not cover

**D214** (tangled units): app/Actions/Application/, app/Actions/Database/, app/Actions/Docker/, app/Actions/Fortify/, app/Actions/ImageScan/, app/Actions/Proxy/, app/Actions/Server/, app/Actions/Service/, app/Actions/Shared/, app/Actions/Stripe/, app/Actions/Team/, app/Actions/TinyHelmTask/, app/Actions/User/, app/Auth/Oidc/, app/Auth/Oidc/Socialite/, app/Data/, app/Enums/, app/Events/, app/Exceptions/, app/Helpers/, app/Http/Controllers/Webhook/Concerns/, app/Http/Middleware/, app/Jobs/, app/Livewire/, app/Livewire/Concerns/, app/Livewire/Server/, app/Models/, app/Models/Scopes/, app/Notifications/, app/Notifications/Application/, app/Notifications/Channels/, app/Notifications/Container/, app/Notifications/Database/, app/Notifications/ImageScan/, app/Notifications/ScheduledTask/, app/Notifications/Server/, app/Notifications/TransactionalEmails/, app/Notifications/VolumeBackup/, app/Policies/, app/Providers/, app/Rules/, app/Services/, app/Services/Access/, app/Services/DeploymentConfiguration/, app/Services/Dns/, app/Services/GithubRunner/, app/Services/ImageScan/, app/Services/ServerTransfer/, app/Support/, app/Support/DatabaseImport/, app/Traits/ import each other (116709 lines). Port them as one piece?

> Yes: one piece in wave 2, no glue between its parts

**D215** (tangled units): app/Mcp/Concerns/, app/Mcp/Resources/, app/Mcp/Servers/, app/Mcp/Tools/ import each other (7777 lines). Port them as one piece?

> Yes: one piece in wave 3, no glue between its parts

**D216** (tangled units): app/Http/Controllers/Api/, app/Http/Controllers/Api/Concerns/ import each other (34331 lines). Port them as one piece?

> Yes: one piece in wave 5, no glue between its parts

**D217** (tangled units): app/Livewire/Project/Service/, app/Livewire/Project/Shared/Storages/ import each other (4803 lines). Port them as one piece?

> Yes: one piece in wave 6, no glue between its parts

