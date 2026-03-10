//! Generate dbt helper macros for DAX-to-SQL constructs.

use crate::error::PbipError;
use std::path::Path;

/// Write all helper macros.
pub fn write_macros(output: &Path) -> Result<(), PbipError> {
    let macros_dir = output.join("macros").join("dax_helpers");

    write_divide_macro(&macros_dir)?;
    write_calendar_macro(&macros_dir)?;
    write_related_macro(&macros_dir)?;

    Ok(())
}

fn write_divide_macro(dir: &Path) -> Result<(), PbipError> {
    let content = r"{#
  DAX DIVIDE helper macro.
  Usage: {{ dax_divide(numerator, denominator, alternate_result=0) }}
  Equivalent to: DIVIDE(numerator, denominator, alternate_result)
#}

{% macro dax_divide(numerator, denominator, alternate_result=0) -%}
    coalesce(
        {{ numerator }} / nullif({{ denominator }}, 0),
        {{ alternate_result }}
    )
{%- endmacro %}
";
    super::write_file(&dir.join("divide.sql"), content)
}

fn write_calendar_macro(dir: &Path) -> Result<(), PbipError> {
    let content = r#"{#
  DAX CALENDAR helper macro.
  Generates a date spine. Requires dbt_utils package.
  Usage: {{ dax_calendar('2020-01-01', '2030-12-31') }}
#}

{% macro dax_calendar(start_date, end_date) -%}
    {{ dbt_utils.date_spine(
        datepart="day",
        start_date="cast('" ~ start_date ~ "' as date)",
        end_date="cast('" ~ end_date ~ "' as date)"
    ) }}
{%- endmacro %}
"#;
    super::write_file(&dir.join("calendar.sql"), content)
}

fn write_related_macro(dir: &Path) -> Result<(), PbipError> {
    let content = r"{#
  DAX RELATED placeholder macro.
  RELATED() in DAX performs a lookup via relationships.
  In dbt, this should be implemented as a JOIN in an intermediate model.
  
  Usage: {{ dax_related('target_model', 'join_key', 'lookup_column') }}
#}

{% macro dax_related(target_model, join_key, lookup_column) -%}
    {# 
      MANUAL_REVIEW: Replace this macro call with a proper JOIN
      in an intermediate model.
      
      Example:
      left join {{ ref(target_model) }} as related_table
        on source.{{ join_key }} = related_table.{{ join_key }}
      
      Then select related_table.{{ lookup_column }}
    #}
    null /* MANUAL_REVIEW: RELATED() requires JOIN — see macro comment */
{%- endmacro %}
";
    super::write_file(&dir.join("related.sql"), content)
}
