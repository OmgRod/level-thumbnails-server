CREATE TYPE enum_role AS ENUM ('user', 'verified', 'moderator', 'admin');

CREATE OR REPLACE PROCEDURE migrate(disc_id bigint, geometry_id bigint)
LANGUAGE plpgsql
AS $$
BEGIN
    -- Step 1: Move uploads
    UPDATE uploads
    SET user_id = disc_id
    WHERE user_id = geometry_id;

    -- Step 2: Migrate data to discord user
    UPDATE users AS u0
    SET
        account_id = u1.account_id,
        username = u1.username,
        role = GREATEST(
                CAST(u0.role AS enum_role),
                CAST(u1.role AS enum_role)
               )::text
    FROM users AS u1
    WHERE u0.id = disc_id
      AND u1.id = geometry_id;

    -- Step 3: Delete Geometry Dash user
    DELETE FROM users
    WHERE id = geometry_id;
END;
$$;