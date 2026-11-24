{{ Illuminate\Mail\Markdown::parse('---') }}

Thank you,<br>
{{ config('app.name') ?? 'TinyHelm' }}

{{ Illuminate\Mail\Markdown::parse('[Contact Support](https://tinyhelm.io/docs/contact)') }}
