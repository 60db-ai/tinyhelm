<?php

namespace App\Actions\Server;

use App\Helpers\SslHelper;
use App\Models\Server;
use App\Models\StandaloneDocker;
use Lorisleiva\Actions\Concerns\AsAction;

class InstallDocker
{
    use AsAction;

    public function handle(Server $server)
    {
        $supported_os_type = $server->validateOS();
        if (! $supported_os_type) {
            throw new \Exception('Server OS type is not supported for automated installation. Please install Docker manually before continuing: <a target="_blank" class="underline" href="https://tinyhelm.com/docs/installation#manually">documentation</a>.');
        }

        if (! $server->sslCertificates()->where('is_ca_certificate', true)->exists()) {
            $serverCert = SslHelper::generateSslCertificate(
                commonName: 'TinyHelm CA Certificate',
                serverId: $server->id,
                isCaCertificate: true,
                validityDays: 10 * 365
            );
            $commands = SslHelper::caCertificateFileCommands($serverCert->ssl_certificate);
            remote_process($commands, $server);
        }

        $config = base64_encode('{
            "log-driver": "json-file",
            "log-opts": {
              "max-size": "10m",
              "max-file": "3"
            }
          }');
        $found = StandaloneDocker::where('server_id', $server->id);
        if ($found->count() == 0 && $server->id) {
            StandaloneDocker::create([
                'name' => 'tinyhelm',
                'network' => 'tinyhelm',
                'server_id' => $server->id,
            ]);
        }
        $command = collect([]);
        if (isDev() && $server->id === 0) {
            $command = $command->merge([
                "echo 'Installing Docker Engine...'",
                "echo 'Configuring Docker Engine (merging existing configuration with the required)...'",
                'sleep 4',
                "echo 'Restarting Docker Engine...'",
                'ls -l /tmp',
            ]);

            return remote_process($command, $server);
        } else {
            if (config('constants.tinyhelm.container_runtime', 'docker') === 'kiri') {
                return $this->installKiri($server, $supported_os_type);
            }
            $command = $command->merge([
                "echo 'Installing Docker Engine...'",
            ]);

            if ($supported_os_type->contains('debian')) {
                $command = $command->merge([$this->getDebianDockerInstallCommand()]);
            } elseif ($supported_os_type->contains('rhel')) {
                $command = $command->merge([$this->getRhelDockerInstallCommand()]);
            } elseif ($supported_os_type->contains('sles')) {
                $command = $command->merge([$this->getSuseDockerInstallCommand()]);
            } elseif ($supported_os_type->contains('arch')) {
                $command = $command->merge([$this->getArchDockerInstallCommand()]);
            } elseif ($supported_os_type->contains('alpine')) {
                $command = $command->merge([$this->getAlpineDockerInstallCommand()]);
            } else {
                $command = $command->merge([$this->getGenericDockerInstallCommand()]);
            }

            $command = $command->merge([
                "echo 'Configuring Docker Engine (merging existing configuration with the required)...'",
                'test -s /etc/docker/daemon.json && cp /etc/docker/daemon.json "/etc/docker/daemon.json.original-$(date +"%Y%m%d-%H%M%S")"',
                "test ! -s /etc/docker/daemon.json && echo '{$config}' | base64 -d | tee /etc/docker/daemon.json > /dev/null",
                "echo '{$config}' | base64 -d | tee /etc/docker/daemon.json.tinyhelm > /dev/null",
                'jq . /etc/docker/daemon.json.tinyhelm | tee /etc/docker/daemon.json.tinyhelm.pretty > /dev/null',
                'mv /etc/docker/daemon.json.tinyhelm.pretty /etc/docker/daemon.json.tinyhelm',
                "jq -s '.[0] * .[1]' /etc/docker/daemon.json.tinyhelm /etc/docker/daemon.json | tee /etc/docker/daemon.json.appended > /dev/null",
                'mv /etc/docker/daemon.json.appended /etc/docker/daemon.json',
                "echo 'Restarting Docker Engine...'",
            ]);
            $command = $command->merge($this->getDockerServiceCommands($supported_os_type->contains('alpine')));
            if ($server->isSwarm()) {
                $command = $command->merge([
                    'docker network create --attachable --driver overlay tinyhelm-overlay >/dev/null 2>&1 || true',
                ]);
            } else {
                $command = $command->merge([
                    dockerProxyNetworkEnsureCommand('tinyhelm'),
                ]);
                $command = $command->merge([
                    "echo 'Done!'",
                ]);
            }

            return remote_process($command, $server);
        }
    }

    /**
     * Install the Kiri container runtime instead of Docker Engine.
     *
     * Kiri speaks the Docker Engine API (reports as Docker 27.5.1 / API 1.47) and its systemd unit
     * creates the compatibility socket /var/run/docker.sock on start, so the rest of the command layer
     * (docker / docker compose / docker inspect over SSH) is unchanged. Kiri is beta (0.2.2); Compose
     * support is a subset of the full spec. Arch/Alpine have no Kiri package yet.
     */
    private function installKiri(Server $server, $supported_os_type)
    {
        if ($supported_os_type->contains('arch') || $supported_os_type->contains('alpine')) {
            throw new \Exception('The Kiri runtime has no package for Arch/Alpine yet. Use a Debian/Ubuntu, RHEL/Fedora, or SUSE server, or set SERVER_CONTAINER_RUNTIME=docker.');
        }

        $sshUser = filled($server->user) ? trim($server->user) : 'root';

        $command = collect([
            "echo 'Installing Kiri container runtime...'",
            // Official installer: detects apt/dnf/zypper/yum, verifies SHA256SUMS-linux, installs the .deb/.rpm.
            'curl -fsSL https://runkiri.com/install.sh | bash',
            "echo 'Starting Kiri service...'",
            'systemctl enable --now kiri',
        ]);

        if ($sshUser !== 'root') {
            // The Kiri Docker API socket is root:kiri 0660; a non-root SSH user needs the kiri group.
            $command->push("usermod -aG kiri {$sshUser} || true");
            $command->push("kiri host-access {$sshUser} || true");
        }

        $command = $command->merge([
            "echo 'Waiting for the Kiri Docker API socket...'",
            // kiri.service creates the /var/run/docker.sock compat link on start; wrapped in sh -c so the
            // loop passes through the non-root sudo parser as one elevated unit.
            "sh -c 'for i in \$(seq 1 30); do [ -S /var/run/docker.sock ] && break; sleep 1; done'",
            'docker version >/dev/null 2>&1 || { echo "Kiri Docker API is not reachable at /var/run/docker.sock" >&2; exit 1; }',
        ]);

        if ($server->isSwarm()) {
            $command->push('docker network create --attachable --driver overlay tinyhelm-overlay >/dev/null 2>&1 || true');
        } else {
            $command->push(dockerProxyNetworkEnsureCommand('tinyhelm'));
        }
        $command->push("echo 'Done!'");

        return remote_process($command, $server);
    }

    private function getDebianDockerInstallCommand(): string
    {
        return 'curl -fsSL https://get.docker.com | sh || ('.
            '. /etc/os-release && '.
            'DOCKER_CODENAME="${UBUNTU_CODENAME:-$VERSION_CODENAME}" && '.
            // Docker has no repository for Ubuntu derivatives (Pop!_OS, Linux Mint) and none for Raspbian Trixie.
            'case "$ID:$DOCKER_CODENAME: $ID_LIKE " in raspbian:trixie:*) DOCKER_REPO=debian ;; *" ubuntu "*) DOCKER_REPO=ubuntu ;; *) DOCKER_REPO=$ID ;; esac && '.
            'install -m 0755 -d /etc/apt/keyrings && '.
            'curl -fsSL https://download.docker.com/linux/${DOCKER_REPO}/gpg -o /etc/apt/keyrings/docker.asc && '.
            'chmod a+r /etc/apt/keyrings/docker.asc && '.
            'echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/${DOCKER_REPO} ${DOCKER_CODENAME} stable" > /etc/apt/sources.list.d/docker.list && '.
            'apt-get update && '.
            'apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin'.
            ')';
    }

    /**
     * Docker has no repository for Amazon Linux, so it gets Docker from its own repository and the Compose plugin from GitHub.
     */
    private function getRhelDockerInstallCommand(): string
    {
        $pluginDirectory = '/usr/local/lib/docker/cli-plugins';

        return "sh -c '. /etc/os-release; if [ \"\$ID\" = amzn ]; then ".
            'dnf install -y docker && '.
            "mkdir -p {$pluginDirectory} && ".
            "curl -fsSL \"https://github.com/docker/compose/releases/latest/download/docker-compose-linux-\$(uname -m)\" -o {$pluginDirectory}/docker-compose && ".
            "chmod +x {$pluginDirectory}/docker-compose; ".
            'else curl -fsSL https://get.docker.com | sh || ('.
            'dnf config-manager --add-repo https://download.docker.com/linux/centos/docker-ce.repo && '.
            'dnf install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin && '.
            'systemctl start docker && '.
            'systemctl enable docker'.
            "); fi'";
    }

    private function getSuseDockerInstallCommand(): string
    {
        return 'curl -fsSL https://get.docker.com | sh || ('.
            'zypper addrepo https://download.docker.com/linux/sles/docker-ce.repo && '.
            'zypper refresh && '.
            'zypper install -y --no-confirm docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin && '.
            'systemctl start docker && '.
            'systemctl enable docker'.
            ')';
    }

    private function getArchDockerInstallCommand(): string
    {
        return 'pacman -Syu --noconfirm --needed docker docker-compose && '.
            'systemctl enable docker.service && '.
            'systemctl start docker.service';
    }

    private function getAlpineDockerInstallCommand(): string
    {
        return 'apk update && '.
            'apk add docker docker-cli-buildx docker-cli-compose && '.
            'mkdir -p /etc/docker';
    }

    private function getDockerServiceCommands(bool $usesOpenRc): array
    {
        if ($usesOpenRc) {
            return [
                'rc-update add docker default',
                'rc-service docker restart',
            ];
        }

        return [
            'systemctl enable docker >/dev/null 2>&1 || true',
            'systemctl restart docker',
        ];
    }

    private function getGenericDockerInstallCommand(): string
    {
        return 'curl -fsSL https://get.docker.com | sh';
    }
}
