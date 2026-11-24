<div>
    <x-slot:title>
        Server Variables | TinyHelm
    </x-slot>

    <x-shared-variables.editor :resource="$server"
        :variables="$server->environment_variables"
        :readOnlyKeys="['TINYHELM_SERVER_UUID', 'TINYHELM_SERVER_NAME']"
        type="server" title="{{ $server->name }}"
        :view="$view" variablesLabel="Server shared variables" />
</div>
