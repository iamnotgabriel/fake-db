## FAKE DB

A lib to help you fake a database for testing.
Using mock in tests, declaring each of the calls a method makes your tests
harder to read and usually noncohesive test data.
With a fake DB, you test your code that depends a repository without making complex calls.
FakeDb comes with most common methods to search, write, delete and update values. All
of that with thread-safe operations and well tested code.

## Usage

Declare a new FakeDb with the the value you want to store as the value , and the primary
key of your FakeDb as key of the HashMap. Declare a new Identifier that will be responsible
for generating new ids.

Declaring Identifier can be boring so always use `fake_db::identifier::Sequential` for auto
incremented values or the macro`fake_db::identifier::impl_identifier` to index by a value of
the stored object.
