# Barnacle

Barnacle is a Discord bot for a World of Warships clan: it schedules Clan Battle events, runs games, and runs the IQ Competition.

## Events

**Event**:
A recurring block of Clan Battle sessions between a first and last day, on its event days, at one start time in one time zone, each a whole number of hours long.

**Session**:
One occurrence of an event on one day.
_Avoid_: Match, game night

**Event days**:
The days of the week an event runs on, chosen only from the days that occur between its first and last day.
_Avoid_: Weekdays

**Template**:
A preset that fills an event's event days, start, length and zone, such as Clan Battles.

**Panel**:
The private message of select menus and buttons where one member builds or edits an event or competition, step by step; typed values such as dates and times go in a form its buttons open.
_Avoid_: Dialog

**Organiser**:
A member who holds one of the server's organiser roles or has the Administrator permission, and so may run /event, /iq's organiser commands and the other organiser commands.
_Avoid_: Admin, moderator

**Member's zone**:
The time zone a member last gave in a time_zone option, remembered for their next panel.
_Avoid_: Server zone

## IQ Competition

**IQ Competition**:
A points competition among the clan, with its own category, an announcements channel, a submissions channel and a judge log; one per server at a time.
_Avoid_: Contest, tournament

**Criterion**:
A named feat worth a fixed number of points that a player can claim by submission.
_Avoid_: Challenge, task

**Submission**:
A message in the submissions channel naming one criterion, with exactly 2 screenshots attached.
_Avoid_: Entry, claim

**Judge**:
A member holding the IQ-Judge role, who awards points by reacting with the Awarded mark and adjusts points by command with a reason.

**Judging marks**:
A competition's three different emojis: Denied, which the bot puts on a malformed submission; Awarded, which a judge adds to a valid submission to award its points; and Confirm awarded, which the bot adds once the points are recorded.
_Avoid_: Judging reaction

**Judge log**:
The competition's third channel, readable only by judges, where the bot records every award, every points change with its reason and new total, and every change to the judges.

**Target role**:
A role whose members can see the competition's channels and are pinged by its announcements.

**Competition template**:
One of the server's 10 newest competitions, whose criteria, rules, roles, judges and judging marks a new competition can copy.

**Archive**:
The state of a cleaned-up competition: its channels are gone, its criteria, rules and scores are kept.
