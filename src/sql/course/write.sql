INSERT INTO courses (
    course_id, aliases, course_title, course_code, course_url, section_title,
    section_code, active, description, logo_img_src, location, meeting_days,
    start_time, end_time, weight
) VALUES (
    :course_id, :aliases, :course_title, :course_code, :course_url, :section_title,
    :section_code, :active, :description, :logo_img_src, :location, :period,
    :order, :hiden, :meeting_days, :start_time, :end_time, :weight
)
