app "movies"
    packages { pf: "cli-platform/main.roc" }
    imports [
        pf.Stdout,
        pf.Stderr,
        pf.Task.{ Task },
        pf.Path.{ Path },
        pf.File.{ FileWriteErr },
        pf.Url.{ Url },
        pf.Http.{ HttpErr },
        pf.Env,
        Encode,
        Json,
    ]
    provides [main] to pf

Movie : {
    title : Str,
    year : U16,
    cast : List Str,
}

exclaim = \title ->
    if Str.endsWith title "!" then
        title
    else
        "\(title)!"


movieFromLine = \line ->
    if Str.isEmpty line then
        Err (InvalidLine "")
    else
        result =
            fields = Str.split line "|"

            title <- List.get fields 0 |> Result.map exclaim |> Result.try
            year <- List.get fields 1 |> Result.try Str.toU16 |> Result.try
            cast <- List.get fields 2 |> Result.try

            Ok { title, year, cast: Str.split cast "," }

        Result.mapErr result \_ -> InvalidLine line

getMovies = \url ->
    response <- Http.getUtf8 url |> Task.await

    response
    |> Str.split "\n"
    |> List.mapTry movieFromLine
    |> Task.fromResult

writeOutput = \movies ->
    path = Path.fromStr "output.json"

    json = List.keepOks movies \movie ->
        when List.first movie.cast is
            Ok starring if movie.year < 1980 ->
                Ok { title: movie.title, starring }

            _ -> Err {}

    File.write path json Json.toUtf8

main =
    task =
        apiKey <- Env.varUtf8 "API_KEY" |> Task.await
        url = Url.fromStr "http://localhost:4000/movies?apiKey=\(apiKey)"
        movies <- getMovies url |> Task.await
        writeOutput movies

    Task.attempt task \result ->
        when result is
            Ok {} -> Stdout.line "Wrote the file!"
            Err (EnvErr _) -> Stderr.line "Could not find API_KEY environment variable"
            Err (HttpErr _) -> Stderr.line "Error reading from URL"
            Err (FileWriteErr _) -> Stderr.line "Error writing to file"
            Err (InvalidLine line) -> Stderr.line "The following line in the response was malformed:\n\(line)"
            Err _ -> Stderr.line "Error!"

expect exclaim "ha!" == "ha!"

expect exclaim "ha" == "ha!"

# decoding title from valid line succeeds
expect
    title =
        movieFromLine "title goes here|1234|first star,second star,third star"
        |> Result.map .title

    title == Ok "title goes here!"

# decoding title from empty line fails
expect
    title =
        movieFromLine ""
        |> Result.map .title

    title == Err (InvalidLine "")
