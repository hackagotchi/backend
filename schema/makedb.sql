-- users of 'gotchi
CREATE TABLE IF NOT EXISTS steaders
    ( steader_id UUID PRIMARY KEY
    , slack_id VARCHAR(16) UNIQUE

    , xp INT NOT NULL
    , extra_land_plot_count INT NOT NULL

    , joined TIMESTAMPTZ NOT NULL
    , last_active TIMESTAMPTZ NOT NULL
    , last_farm TIMESTAMPTZ NOT NULL
    );
-- necessary for operation through slack client
-- (doesn't do much anything for slackless users who never log in that way)
CREATE INDEX ON steaders (slack_id);

-- items (owned by steaders)
CREATE TABLE IF NOT EXISTS items 
    ( item_id UUID PRIMARY KEY
    , owner_id UUID NOT NULL REFERENCES steaders (steader_id) ON DELETE CASCADE

    , archetype_handle INT NOT NULL
    );
CREATE TABLE IF NOT EXISTS ownership_logs 
    ( item_id UUID NOT NULL REFERENCES items (item_id) ON DELETE CASCADE
    , logged_owner_id UUID NOT NULL REFERENCES steaders (steader_id) ON DELETE CASCADE

    -- the fourth person to own this? the third? nineteenth?
    , owner_index INT NOT NULL
    -- farmed it? traded? from an egg?
    , acquisition INT NOT NULL
    );
CREATE TABLE IF NOT EXISTS gotchi 
    ( item_id UUID NOT NULL REFERENCES items (item_id) ON DELETE CASCADE
    , nickname VARCHAR(64) NOT NULL
    );

-- tiles & plants (owned by steaders)
CREATE TABLE IF NOT EXISTS tiles 
    ( tile_id UUID PRIMARY KEY
    , owner_id UUID NOT NULL REFERENCES steaders (steader_id) ON DELETE CASCADE
    , acquired TIMESTAMPTZ NOT NULL DEFAULT (NOW() AT TIME ZONE 'utc')
    );
CREATE TABLE IF NOT EXISTS plants
    ( tile_id UUID NOT NULL REFERENCES tiles (tile_id) ON DELETE CASCADE
    , xp INT NOT NULL
    , nickname VARCHAR(64) NOT NULL
    , until_yield FLOAT NOT NULL
    , archetype_handle INT NOT NULL
    );
CREATE TABLE IF NOT EXISTS plant_crafts
    ( tile_id UUID NOT NULL REFERENCES tiles (tile_id) ON DELETE CASCADE
    , until_finish FLOAT NOT NULL
    , recipe_archetype_handle INT NOT NULL
    );
CREATE TABLE IF NOT EXISTS plant_effects
    ( tile_id UUID NOT NULL REFERENCES tiles (tile_id) ON DELETE CASCADE
    , until_finish FLOAT -- not all effects have an expiration date
    , item_archetype_handle INT NOT NULL
    , effect_archetype_handle INT NOT NULL
    );
