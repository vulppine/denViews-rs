Subfolder Table
|FolderID|ParentID|FolderName|

FolderID - PRIMARY KEY - Identifies every folder.
ParentID - FOREIGN KEY - Identifies a folder's parent.
FolderName - STRING, NOT NULL - Identifies a folder's name.

FolderID 0 will always be reserved for the root of a website.

Page Table
|PageID|FolderID(F)|PathID(F)|PageName|FirstVisited|TotalViews|TotalHits|

PageID - PRIMARY KEY - Identifies every page.
FolderID - FOREIGN KEY - Identifies the folder a page is in.
PathID - FOREIGN KEY - Identifies the path ID a page has for faster view
grabbing.
PageName - STRING - Identifies the name (canonical) of a page.
FirstVisited - DATE - Identifies the date a page was first visited.
TotalViews - NUMBER - Identifies the amount of unique views a page has.
TotalHits - NUMBER - Identifies the total number of views a page has.

Full Paths Table
|PathID|Path|

PathID - PRIMARY KEY - Identifies a path.
Path - STRING - The path's full name.

Visitors Table
|VisitorID|DateVisited|Country|

VisitorID - PRIMARY KEY - A hash of the visitor, depending on several aspects of
the visitor themselves. This is most likely going to be a combination of:
- IP Address
- Browser User-Agent
- A unique hash, reset every week
DateVisited - DATE - The first date this visitor has visited.
Country - STRING - The country this visitor is visiting from. This might not be
needed.

Hash Table
|Hash|

Hash - STRING - Purely here for reference only, and for easy fetching. This is
not an actual table.

PageVisitors Table
|VisitorID(F)|PageID(F)|TimesVisited|

VisitorID - FOREIGN KEY - See above.
PageID - FOREIGN KEY - See above.
TimesVisited - NUMBER - The amount of times a visitor has visited the page. This
is unique to PageVisitors, and will be calculated along with TotalViews when the
Visitors table is flushed.

Settings (misc, not really needed in schema)

Views

View: Path[i]

Every path will have a unique view associated with it, numbered by which PathID
it currently has. This view will calculate both Views and Hits both from the
PageVisitors table, and the current record, and return it as a single row.
