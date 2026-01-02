# Integrations

# Status

**DRAFT**

# GitLab

GitLab provides an [Achievements API](https://docs.gitlab.com/ee/user/profile/achievements.html). It
appears it internally manages a database of achievements, and provides them their own ID when they
are created. So we'll have to maintain a mapping of our achievement IDs to the GitLab instance's
IDs.

This implies that there needs to be a stateful database that is GitLab (or in general) integration
specific.

It does not appear that you can link an achievement to the commit that generated it.

It'd be sweet to run this through a pipeline, although that'd require finding some way to have
persistent storage, especially caching the bare repositories. Maybe ask Chris D. about effective
ways to do this.

# Generate a static web page suitable for GitHub / GitLab pages sites

This leans into the idea of Herostratus's primary use-case being run in CI/CD pipelines. Can I host
the database as a static asset and make the site dynamic? Or do I just need to regenerate a static
site each time?

* Custom CSS
* Allow inserting custom HTML header/footer similar to docs.rs
* Landing page:
  * Repository list with summary of users/achievements
  * List of users, each of which gets a page
    * Summary (number of commits? per repository? activity graph?)
    * List of achievements
  * Link to achievement list (and link to users who have them?)
  * Link back to Herostratus' GitHub repo

Each achievement would need its own icon; perhaps take inspiration from Acha? I bet GenAI could do a
decent job of helping this unartistic fellow generate them.

This might be more appealing than integrating with GitLab's Achievements API directly, especially
for a whimsical project like this ($WORK hates fun, and hosting your own site doesn't have the
appearance of being "official"). An advantage of the GitLab approach though, is that it emails users
when they get an achievement, which is also perhaps a disadvantage given how silly I want the
achievements to be.

# Spit out achievements as JSON over stdout

This could be an adapter integration that could be useful for integration tests, or for users to
build their own integrations against if they don't want to use the trait-and-feature API.

# Integration API

The integration is _the_ thing the users of Herostratus care about. So I think making it easy to
implement / stand up is important.

* Cargo feature with trait-defined interface
  * I think this is my favorite? It makes sharing the achievements and user database easier? Or at
    least it makes the interface more clear / well documented. But it does require recompilation
    (not a big deal) or including the integration in Herostratus itself (more of a big deal).
* JSON on stdout
* REST API with `/grant` `/revoke` etc. endpoints

The API should include:

* have typed access to the user database
* have typed access to the achievements database
* have typed access to the list of all possible achievements
* have raw access to the database for its own uses
* handle granting an achievement
* handle revoking an achievement

Should it also be able to configure the repositories that Herostratus scans? That's a more complex
use-case, but it enables using Herostratus as the backend for a custom frontend (something more like
Acha).
