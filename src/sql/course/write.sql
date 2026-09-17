INSERT INTO courses (
    course_id, aliases, course_title, course_code, course_url, section_title,
    section_code, active, description, logo_img_src, location, meeting_days,
    start_time, end_time, weight
) VALUES (
    :course_id, :aliases, :course_title, :course_code, :course_url, :section_title,
    :section_code, :active, :description, :logo_img_src, :location, :meeting_days,
    :start_time, :end_time, :weight
) ON CONFLICT(course_id) DO UPDATE SET
    aliases = excluded.aliases, course_title = excluded.course_title,
    course_code = excluded.course_code, course_url = excluded.course_url,
    section_title = excluded.section_title, section_code = excluded.section_code,
    active = excluded.active, description = excluded.description,
    logo_img_src = excluded.logo_img_src, location = excluded.location,
    meeting_days = excluded.meeting_days, start_time = excluded.start_time,
    end_time = excluded.end_time, weight = excluded.weight
