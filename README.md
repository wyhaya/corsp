
# corsp

> A simple [CORS](//developer.mozilla.org/docs/Web/HTTP/CORS) proxy tool

## Usage

### Start

```sh
corsp -b 1080
# [INFO ] Serving address: 0.0.0.0:1080
```

### Change request

```
http://example.com/api/books
->
http://0.0.0.0:1080/http://example.com/api/books
```

