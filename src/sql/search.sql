SELECT
    m.item_type,
    m.item_id,
    m.course_id,
    c.course_title,
    f.title,
    snippet(search_index, 1, '', '', '…', 20) AS preview,
    bm25(search_index, 5.0, 1.0) AS score
FROM search_index f
JOIN search_mapping m ON m.id = f.rowid
JOIN courses c ON c.course_id = m.course_id
WHERE search_index MATCH ?
ORDER BY score, m.id
LIMIT 30;
