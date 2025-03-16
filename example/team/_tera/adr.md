{% macro header_table() -%}

<table>
  <tr>
    <th>Author</th>
    <td>{{ metadata(path="author") }}</td>
  </tr>
  <tr>
    <th>Status</th>
    <td>{{ self::status() }}</td>
  </tr>
</table>

{%- endmacro %}

{% macro status() -%}
<ac:structured-macro ac:name="status" ac:schema-version="1" ac:macro-id="6d70f52a-bd50-4006-ac6c-5da9643e207a">
<ac:parameter ac:name="title">{{ metadata(path='status') }}</ac:parameter>
<ac:parameter ac:name="colour">{%- if metadata(path='status') == 'new' -%}
Black
{%- elif metadata(path='status') == 'in progress' -%}
Yellow
{%- elif metadata(path='status') == 'on track' -%}
Green
{%- elif metadata(path='status') == 'at risk' -%}
Red
{%- else -%}
Blue
{%- endif -%}</ac:parameter>
</ac:structured-macro>
{% endmacro -%}
