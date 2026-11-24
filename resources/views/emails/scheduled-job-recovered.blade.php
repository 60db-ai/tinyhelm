<x-emails.layout>
{{ $title }}

The last run succeeded after failed runs.

@if ($url)
Click [here]({{ $url }}) to open it in TinyHelm.
@endif
</x-emails.layout>
