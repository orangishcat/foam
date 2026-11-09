INSERT INTO tasks (
    folder_id, id, source, title, description, due, attachments
) VALUES (
    :folder_id, :id, COALESCE(:source, 'builtin'), :title, :description, :due, :attachments
) ON CONFLICT(folder_id, id) DO NOTHING