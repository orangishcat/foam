INSERT INTO events (id, title, description, due, source) VALUES (
    :id, :title, :description, :due, :source
) ON CONFLICT(id) DO NOTHING
