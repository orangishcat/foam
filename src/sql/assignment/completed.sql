-- SQL query equivalent of the is_completed function in Assignment in types/assignment.rs
COALESCE(
    manual_mark,
    (julianday(due) < julianday('now') AND NOT allow_submissions)
    OR json_array_length(submissions) > 0
    OR score IS NOT NULL
    OR letter_grade IS NOT NULL
)
