# Integrations

# Status

**DRAFT**

# GitLab

GitLab provides an [Achievements API](https://docs.gitlab.com/ee/user/profile/achievements.html).
It appears it internally manages a database of achievements, and provides them their own ID when
they are created. So we'll have to maintain a mapping of our achievement IDs to the GitLab
instance's IDs.

This implies that there needs to be a stateful database that is GitLab (or in general) integration
specific.

It does not appear that you can link an achievement to the commit that generated it.

It'd be sweet to run this through a pipeline, although that'd require finding some way to have
persistent storage, especially caching the bare repositories. Maybe ask Chris D. about effective
ways to do this.
