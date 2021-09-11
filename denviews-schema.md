Subfolder Table
[FolderID][ParentID][FolderName]

e.g.,

 0 NULL root
 1  0   blog
 2  1   post
 3  2    1
 4  2    2

Page Table
[PageID][FolderID][PathID][PageName][FirstVisited][TotalViews][TotalHits]

e.g.,

 0 0 index.html
 1 1 index.html
 2 1 archive.html
 3 3 index.html
 4 4 index.html

TotalViews and TotalHits **cannot be avoided**. These are to avoid stressing out
the RDBMS with massive selects on what is practically a two-column table. These
are not calculated values based on any existing values, as that table has all
its rows deleted periodically and flushed into every record's respectful
columns as the original value plus the new, calculated value.

PathID references a path's full ID for fetching a view without having to
recreate it, where the view stores a query that fetches the amount of entries
currently in UniqueVisitors with a page's ID, the sum of all hits within that
page's TimesVisited column, plus the respectful columns within that PageID on
the Page table.

These two combine to become something like:

root/blog/post/1/index.html
 0    1    2   3   3<- PageID
root/blog/post/2/index.html
 0    1    2   4   4<- PageID
root/blog/archive.html
 0    1    2<- PageID

The goal is to find something that calculates all the above values, joins them
with a slash, and outputs them for viewing. This is possible via several
SELECTs if needed, but it's more likely a view will be created with the given
page's path, which then calculates it based on the many-many relationship
between visitors and pages (described below). This is just here for when the
*database* must be accessed via some administration tool (which is an entirely
different implementation detail), and sections need to be accessed as cleanly as
possible.

Key 0 in the `subfolders` table will *always* be reserved for the root of the
website.

So, views will be stored like this:

root_blog_post_2_index.html

Alternatively, full paths can be stored in a separate table, and then fetched to
get a key to avoid running into the SQL table name length restriction, so:

FullPaths
[PathID][Path]
   0      root/blog/post/2/index.html

View: Path0
...information...

It's a hack, but it does work.

Paths are only created when a new path is requested, and when requested from the
resultant web server, it returns a 200 OK - if it does **not** return a 200 OK,
the path will not be created, and an internal/user error will be returned to the
requester.

Paths will always be requested by exact details before the web server is
checked. If the path exists, it will continue with processing. Equally, the view
will *always* have the PageID, meaning that adding it to the UniqueVisitors
relationship table will be as simple as doing an INSERT operation.

A unique view, TotalViews, will be kept in order to simplify a flush to table
operation where all pages are iterated over that have views, and then deleted
from the UniqueVisitors table in order to completely clear out the table.

VisitorsTable
[VisitorID]

VisitorID is a hashed value that will occasionally be cleared.

This is just to establish unique records -> pageIDs for the many-many
relationship. This will be periodically cleared to adhere to GDPR requirement,
and will result in null-PageID links - this cannot be helped if the pageID is to
be indexed properly.

The better thing to do would be to put a field in the Page Table in order to
avoid the potential issues related to having UniqueViews become a large table
that is periodically indexed...

...however, aren't RDBMSes supposed to be able to perform on thousands of rows?

UniqueViews
[VisitorID-F][PageID-F][TimesVisited]
   ^
   |
   Can be null/default valued, to adhere to GDPR requirement
   VisitorID 0 will *always* be reserved, as a result. No
   visitor can be visitor 0.

Perhaps UniqueViews can also be flushed to PageID's row in a calculated value -
even though this is *supposed* to be avoided, it **prevents an entire table full
of null values from being indexed several times over**. UniqueViews can then be
summed with the PageID's internal value in order to get the accurate number of
views per unique page.

TimesVisited is a per-record column that indicates how many times a visitor has
visited a page. This will be summed along with the amount of records to indicate
the total number of 'hits' a page has, versus total number of 'views'.

Settings Table
[Setting][Value]

Setting is a primary key, Value is any valid JSON string. This will be
interpreted in the API wrapper when it loads.
