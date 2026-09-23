# specs

Machine authority: [`component.spec.json`](component.spec.json).

OpenAI-compatible provider adapter implementing the SPI `EmbeddingModelPort`
and `LanguageModelPort`. Deployment-optional: constructed only when
`SDKWORK_MEMORY_OPENAI_API_KEY` is configured.
