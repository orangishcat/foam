INSERT INTO events (id, title, description, due, source) VALUES (
    :id, :title, :description, :due, :source
) ON CONFLICT(id) DO UPDATE SET
    title = excluded.title,
    description = excluded.description,
    due = excluded.due,
    source = excluded.source
