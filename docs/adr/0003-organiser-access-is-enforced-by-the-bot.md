# Organiser access is enforced by the bot, with organiser commands listed for everyone

Each server names its organiser roles with /barnacle access, and a member may run /event, /iq's organiser subcommands, /rehearse, /updates, /announce and /voice when they hold one of those roles or have the Administrator permission; with no roles named, only Administrators may. Discord's default member permissions cannot express "holds one of these roles", so those commands carry no default permission, every member sees them in the command list, and one bot-side check refuses anyone else privately, with the final writing button of each panel checking again. Keeping Discord's permissions was rejected because an Administrator would have to edit every command's integration settings by hand, and hiding the commands would leave non-admin organisers unable to find them.

Status: accepted
