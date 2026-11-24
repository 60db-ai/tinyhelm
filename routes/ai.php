<?php

use App\Mcp\Servers\TinyHelmServer;
use Laravel\Mcp\Facades\Mcp;

Mcp::web('/mcp', TinyHelmServer::class)
    ->middleware(['mcp.enabled', 'api.ip.allowed', 'auth:sanctum', 'api.token.team', 'mcp.team.enabled']);
