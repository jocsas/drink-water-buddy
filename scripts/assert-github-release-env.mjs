function fail(message) {
  console.error(`\nGitHub release blocked: ${message}\n`);
  process.exit(1);
}

if (!process.env.GH_TOKEN && !process.env.GITHUB_TOKEN) {
  fail('set GH_TOKEN or GITHUB_TOKEN with permission to create releases and upload release assets.');
}

console.log('GitHub release prerequisites look OK.');
