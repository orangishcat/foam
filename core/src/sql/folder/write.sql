INSERT INTO folders (
    folder_id, source, parent_id, title, custom_icon, data
) VALUES (
    :folder_id, COALESCE(:source, 'builtin'), '', :title, :custom_icon, :data
) ON CONFLICT(folder_id) DO UPDATE SET
    title = excluded.title,
    custom_icon = excluded.custom_icon,
    data = excluded.data