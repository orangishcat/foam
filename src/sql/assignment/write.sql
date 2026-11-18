INSERT INTO assignments (
    course_id, id, title, description, due, manual_mark, max_points, score,
    letter_grade, allow_submissions, attachments, submissions
) VALUES (
    :course_id, :id, :title, :description, :due, :manual_mark, :max_points, :score,
    :letter_grade, :allow_submissions, :attachments, :submissions
) ON CONFLICT(course_id, id) DO UPDATE SET
    title = excluded.title,
    description = excluded.description,
    due = excluded.due,
    max_points = excluded.max_points,
    score = excluded.score,
    letter_grade = excluded.letter_grade,
    allow_submissions = excluded.allow_submissions,
    attachments = excluded.attachments,
    submissions = excluded.submissions
