# syntax=docker/dockerfile:1.7
FROM mcr.microsoft.com/dotnet/sdk:10.0.302-alpine3.23@sha256:d8ee39817ca03a3757288e83c37ed73cc969a286c603b827c7cbe33add1c2d1c AS build
WORKDIR /src
COPY Sapphirus.DesktopSupportApi.csproj packages.lock.json ./
RUN dotnet restore --locked-mode
COPY . ./
RUN dotnet publish --no-restore --configuration Release --output /out \
    /p:UseAppHost=false /p:PublishReadyToRun=false

FROM mcr.microsoft.com/dotnet/aspnet:10.0.11-alpine3.23@sha256:4d5339ac9814f1a033a09e664bcf159e9fb386c89b6e7917b3dca7254e656027 AS runtime
WORKDIR /app
ENV ASPNETCORE_HTTP_PORTS=8080 \
    DOTNET_EnableDiagnostics=0 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
USER $APP_UID
COPY --from=build --chown=$APP_UID:$APP_UID /out ./
ENTRYPOINT ["dotnet", "Sapphirus.DesktopSupportApi.dll"]
