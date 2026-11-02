const http = require('node:http');
const port = Number(process.env.PORT || 4317);
const server = http.createServer((_request, response) => {
  response.setHeader('Content-Type', 'application/json');
  response.end(JSON.stringify({
    profile: process.env.APP_PROFILE || 'unset',
    greeting: process.env.GREETING || 'Hello',
    pid: process.pid,
    port,
  }));
});
server.on('error', (error) => {
  console.error(`Server error: ${error.code}`);
  process.exitCode = 1;
});
server.listen(port, '127.0.0.1', () => console.log(`Demo listening at http://127.0.0.1:${port} (${process.env.APP_PROFILE})`));
process.on('SIGTERM', () => server.close(() => process.exit(0)));
process.on('SIGINT', () => server.close(() => process.exit(0)));
