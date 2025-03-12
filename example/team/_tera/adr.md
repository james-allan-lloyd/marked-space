{% macro header_table() -%}

<table>
  <tr>
    <th>Author</th>
    <td>{{ metadata(path="author") }}</td>
  </tr>
  <tr>
    <th>Status</th>
    <td>{{ metadata(path="status") }}</td>
  </tr>
</table>

{%- endmacro %}
