SELECT
    m.item_type,
    m.item_id,
    m.folder_id,
    m.source,
    s.title,
    COALESCE(json_extract(f.data, '$.logo_img_src'), '') AS logo_img_src,
    COALESCE(f.title, 'Unknown source') AS folder_title,
    snippet(search_index, 1, '', '', '…', 20) AS preview,
    bm25(search_index, 5.0, 1.0, 0.0, 0.1) AS score
FROM search_index s
JOIN search_mapping m ON m.id = s.rowid
LEFT JOIN folders f ON f.folder_id = m.folder_id
WHERE search_index MATCH ?
ORDER BY score, m.id
LIMIT 30;
