# Version 2
Added X-LAST-UPDATED which is equal to the unix timestamp in seconds when the calendar was last generated
Added X-UPDATE-INTERVAL-SECONDS which is equal to KUMA_HEARTBEAT_INTERVAL env variable signifying how often the calendar should be updated
Updated X-BUSSIE-METADATA to always be equal to the base shift, not a modified version. So two events of a broken shift both now have the same X-BUSSIE-METADATA value
Added X-CAL-VERSION which signifies what version from this changelog the calendar is based on