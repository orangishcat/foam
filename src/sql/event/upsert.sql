INSERT INTO events (id, title, subtitle, description, due, source, course_id) VALUES (
    :id, :title, :subtitle, :description, :due, :source, :course_id
) ON CONFLICT(id) DO UPDATE SET
    title = excluded.title,
    subtitle = excluded.subtitle,
    description = excluded.description,
    due = excluded.due,
    source = excluded.source,
    course_id = COALESCE(excluded.course_id, events.course_id)
