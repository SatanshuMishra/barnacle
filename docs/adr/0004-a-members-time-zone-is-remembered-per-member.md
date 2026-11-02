# A member's time zone is remembered per member, not detected or set per server

Discord gives a bot no member's time zone, and a menu holds only 25 of the hundreds of zones. So /event create, /event edit and /iq start take an optional time_zone option that searches every zone by city; the zone a member gives is remembered for that member across every server and used for a new event or competition whenever they leave the option out, and a member with none is asked once; editing an event changes its zone only when the option is given. A server-wide zone was rejected because clan members play from several zones, and guessing from the Discord locale was rejected because a language does not name a zone.

Status: accepted
