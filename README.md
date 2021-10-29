# Rust TCP-Chat Example
## About

This is a simple TCP-chat in Rust (a university home assignment).
It illustrates the use of both the blocking and the non-blocking approaches to communication implementation.

The chat works as a room that random clients may connect to, and implements:
* Sending messages (with the sender's name & timestamp)
* Notifying clients about events (new client connects, someone disconnects, etc.)
* Uploading files to the server
* Downloading files from the server

## Build

```bash
cargo build
```

Configure the `target` folders with either `configure.bat` or `./configure.sh`.

## Run

```bash
cargo run
```

The server can be stopped via `Ctrl-C`.

### Client Commands

The client app supports the following commands:

#### `/quit`, `/exit`, `/q`

Stop the client app.

#### `/connect [address] [port]`, `/c`

Establishes the connection with the server.

The default `address` is `localhost`.

The default `port` is 6969.

#### `/rename <new_name>`, `/r`

Asks the server to change the name of the current user.

#### `/upload <name> [local_path]`, `/u`

Upload a file to the server.
Ask the server to save it as `name`, and locally take it from the `local_path`.

The default `local_path` equals `name`.

#### `/download <name> [local_path]`, `/d`

Download a file `name` from the server.
Save it locally to the `local_path`.

The default `local_path` equals `name`.

#### `<text>`

Sends a text message to the server.

## Protocol

The protocol assumes communication via _messages_: short pieces of data with predefined formats.

A single message cannot exceed some fixed number of bytes in size (currently `MAXIMUM_MESSAGE_SIZE = 1024`).
If a receiver can't parse a message within this amount of bytes, the connection must be dropped.

There're multiple message formats in use.
Each one corresponds to some high-level situation (since we design the protocol for a single use case - a chat - it's aware of the context). In general, they are side-specific.

Some messages are self-sufficient, others represent a single step in a more complication communication procedure.

The full list of currently used message formats can be found in `messages.rs`.

### Common Message Formats
#### `Chunk { data: Vec<u8>, id: usize }`

Represents a piece of contiguous binary `data`.
The `id` field determines the context (some _larger thing_ this data relates to).

`Chunk` messages are used for sending files _to_ and _from_ the server.

### Client Message Formats
#### `Text { text: String }`

A text message a client sends to the server.
It's the server's responsibility to determine the client's name and time details.
Server then broadcasts its own `Text` message with all the details.

#### `Leave`

Notifies the server about the client's intent to leave. The server closes its side of the connection upon receiving a message.

#### `Rename { new_name: String }`

Asks the server to set a new name for the current client.

If the new name has been accepted, the server broadcasts a `UserRenamed` message.
Otherwise, a `Support` message is sent back with the explanation of what went wrong.

#### `RequestFileUpload { name: String, size: usize, id: usize }`

Asks the server if it can accept a file named `name` of the specified `size`.

If the server can accept it, the `id` is used to refer to this file transfer procedure (as opposed to transferring other files if they are sent simultaneously).
In this case, the server sends back an `AgreeFileUpload`.

Otherwise, a `DeclineFileUpload` is returned.

#### `RequestFileDownload { name: String }`

Asks the server if it can send a file named `name`.

If so, the server returns `AgreeFileDownload` with the corresponding `size` and `id`.

Otherwise, `DeclineFileDownload` is sent.

#### `AgreeFileDownload { id: usize }`

Notifies the server that the client is still wiling to accept the file after taking into account its `size`.

After this message the server starts to actually send `Chunk`s.

#### `DeclineFileDownload { id: usize }`

Notifies the server that the client is not willing to accept the file anymore (e.g. the size of the file is too big for the client machine or something).

The server cancels the file transfer after this message.

### Server Message Formats

#### `Text { text: String, name: String, time: DateTime }`

The message the server broadcasts when it wants to send a text message to everyone. The `name` and the `time` are determined by the server according to the contextual information.

#### `NewUser { name: String, time: DateTime }`

A notification meaning there's a new client in the room.

#### `Interrupt { name: String, time: DateTime }`

When a user suddenly disconnects (without sending a `Leave` message), the server assumes it's due to some error with the client, and broadcasts this notification.
This means, the client disconnects, but they might have not wanted to do it.

#### `UserLeaves { name: String, time: DateTime }`

This notification means the client disconnects from the room normally.

#### `Support { text: String }`

A message containing some technical details.
Typically sent on a per-client basis to explain something.

#### `UserRenamed { old_name: String, new_name: String }`

A notification meaning a user successfully changed their name.

#### `NewFile { name: String }`

A notification that means someone has uploaded a new file.

#### `AgreeFileUpload { id: usize }`

A message the server sends back to the client who have requested a file uploading procedure (see the `RequestFileUpload` client message) in case if such a file can be accepted by the server.

After receiving this message, the client starts to actually send the `Chunk`s.

#### `DeclineFileUpload { id: usize, reason: String }`

A message meaning the server cannot accept the specified file.

Sent by the server after receiving the corresponding `RequestFileUpload` client message.

After this message the client cancels the sending procedure.

#### `AgreeFileDownload { name: String, size: usize, id: usize }`

A message meaning the server can give the client the file they have requested via the corresponding `RequestFileDownload` message.

Upon receiving this message, the client has to decide whether the given file information (`size`) is fine for them and either return an `AgreeFileDownload` or a `DeclineFileDownload` (the client-side versions).

#### `DeclineFileDownload { name: String, reason: String }`

A message the server returns if the requested file cannot be returned (e.g. not found or something).

The client cancels the file transfer procedure after this message.

### Serialization

The above message formats are translated into sequences of bytes via a BSON-serializer.
These sequences of bytes are then written/read to/from the sockets.

**Note.** As I realized after having done the first part of the lab, I shouldn't have used a ready-made solution, so if you're building such a tcp-chat yourself, consult the teacher to clear things out.

### File Transfer Overview

Let's go once more over how a successful file transfer would look.

Client Uploads File:

```
Client -> RequestFileUpload -> Server
Client <-  AgreeFileUpload  <- Server
Client ->       Chunk+      -> Server
```

Client Downloads File:

```
Client -> RequestFileDownload -> Server
Client <-  AgreeFileDownload  <- Server
Client ->  AgreeFileDownload  -> Server
Client <-        Chunk+       <- Server
```

The end of the file transfer is handled by the receiver by the check that the number of already accepted bytes equals the initially sent `size`.

### Blocking vs Non-Blocking

Currently, the server works with the sockets in a blocking way, and the client works with them in a non-blocking manner.
There's no deep reasoning to it, it's just a way to illustrate that both approaches are possible.

The problem of time wasted during iterations in the non-blocking approach is solved by checking whether there was some work to do during the previous iteration.
This allows to save processor time for slow communication, but still utilize maximum performance when put under pressure.
On the other hand, there's still a small initial delay after an iteration of doing nothing.

## Links

* Formal requirements: https://insysnw.github.io/practice/hw/tcp-chat/
