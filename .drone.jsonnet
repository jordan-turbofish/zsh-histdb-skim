local Pipeline(name, image) = {
  kind: "pipeline",
  type: "docker",
  name: name,
  steps: [
    {
      name: "test",
      image: image,
      pull: "if-not-exists",
      commands: [
        "cargo build --verbose --all --release",
        "mkdir dist",
        "cp target/release/zsh-histdb-skim dist/zsh-histdb-skim-linux-x64",
        "cargo test --verbose --all"
      ]
    },
    {
      name: "release",
      image: "plugins/github-release",
      pull: "if-not-exists",
      settings: {
        api_key: {
          from_secret: "github_release",
        },
        files: [
          'target/release/zsh-histdb-skim',
          'dist/*',
        ],
        draft: true,
      },
      when: {
        event: 'tag'
      },
    }
  ]
};

[
  Pipeline("rust-1-59", "rust:1.59"),
]
