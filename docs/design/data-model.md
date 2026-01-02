# Data model

# Status

**PROPOSAL**

# Scope

This document answers the following questions

* What data does an achievement contain or reference?
* What inputs are required for a Rule engine?

# Achievements

## Achievement uniqueness

1. Repeatable. E.g., swear in a commit message.
2. Globally Unique. E.g., longest/shortest commit message.

## Achievement contents

* Achievement ID

  This can be used to look up the title, description, art, etc. Or the title and description could
  be included in the achievement? (That would also support embedding metadata / using custom titles
  or descriptions).

  * Title
  * Description
  * If the achievement linked to its icon, that could enable custom icons per achievement, but I
    think I'd rather just have the icon based on the achievement ID.
* User ID

  This needs to be .mailmap aware, and may need to have committer/author distinction?
* What repository the achievement is associated with
  * How to handle if Herostratus is run on multiple branches of the same repository? Or even an SSH
    and HTTPS URL of the same repository?
* What commit the achievement is associated with
* Achievement uniqueness

  A consumer of the rule engine will consume this, and determine whether it needs to revoke the
  achievement from another user to grant it to another one.
