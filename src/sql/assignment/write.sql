INSERT INTO assignments (
    course_id, id, title, description, due, manual_mark, max_points, score,
    letter_grade, allow_submissions, attachments, submissions
) VALUES (
    :course_id, :id, :title, :description, :due, :manual_mark, :max_points, :score,
    :letter_grade, :allow_submissions, :attachments, :submissions
) ON CONFLICT(course_id, id) DO NOTHING
