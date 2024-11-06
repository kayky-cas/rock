# Rock

Exemple of a config file
```json
{
    "proxy": {
        "host": "google.com",
        "port": 443 /* or 80 to http */
    },
    "responses": [
        {
            "path": "/",
            "method": "get",
            "enabled": true,
            "status": 200,
            "body": {
                "message": "hello"
            }
        },
        {
            "path": "/hello/{a}",
            "method": "get",
            "enabled": true,
            "status": 200,
            "body": {
                "message": "hello to {a}"
            }
        }
    ]
}
```

## Usage

```bash
rock -p 3000 -f config.json
```
```bash
curl localhost:3000
{"message":"hello"}%
```
```bash
curl localhost:3000/hello/kayky
{"message":"hello to kayky"}%
```
```bash
curl localhost:3000/images
# will be redirect to google.com/images
```
