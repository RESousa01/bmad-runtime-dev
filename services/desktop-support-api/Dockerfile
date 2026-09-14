# syntax=docker/dockerfile:1.7
FROM mcr.microsoft.com/dotnet/sdk:10.0.302-alpine3.23@sha256:d8ee39817ca03a3757288e83c37ed73cc969a286c603b827c7cbe33add1c2d1c AS build
WORKDIR /src
COPY Sapphirus.DesktopSupportApi.csproj packages.lock.json ./
RUN dotnet restore --locked-mode
COPY . ./
RUN dotnet publish --no-restore --configuration Release --output /out \
    /p:UseAppHost=false /p:PublishReadyToRun=false

FROM mcr.microsoft.com/dotnet/aspnet:10.0.12-alpine3.23@sha256:68eeb5922e4737a0f4657e3a631e566c38186f910c7155fb7c377414ce58a64d AS runtime
WORKDIR /app
ENV ASPNETCORE_HTTP_PORTS=8080 \
    DOTNET_EnableDiagnostics=0 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
USER $APP_UID
COPY --from=build --chown=$APP_UID:$APP_UID /out ./
ENTRYPOINT ["dotnet", "Sapphirus.DesktopSupportApi.dll"]
