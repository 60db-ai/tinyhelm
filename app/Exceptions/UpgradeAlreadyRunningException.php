<?php

namespace App\Exceptions;

use App\Services\TinyHelmUpgradeStatus;
use Exception;

/**
 * An upgrade was not started because another upgrade script is still running.
 * This is not a failure of the running upgrade.
 */
class UpgradeAlreadyRunningException extends Exception
{
    public function __construct()
    {
        parent::__construct(
            'Another TinyHelm upgrade is already running. Wait for it to finish. '.
            'If it has stopped, you can upgrade again '.TinyHelmUpgradeStatus::RUNNING_LOCK_EXPIRES_AFTER_MINUTES.' minutes after its last status update.'
        );
    }
}
