# syntax=docker/dockerfile:1.7
FROM mcr.microsoft.com/dotnet/sdk:10.0.400-alpine3.23@sha256:b36516b249f0cccf9e5017082f51d4bda2d61469f205a7167fbf3b8498ecdd59 AS build
WORKDIR /src
COPY Sapphirus.DesktopSupportApi.csproj packages.lock.json ./
RUN dotnet restore --locked-mode
COPY . ./
RUN dotnet publish --no-restore --configuration Release --output /out \
    /p:UseAppHost=false /p:PublishReadyToRun=false

FROM mcr.microsoft.com/dotnet/aspnet:10.0.10-alpine3.23@sha256:27b6b84beeede74fd16886177d360799c8e4299ceadfbd64eef57bafead7878a AS runtime
WORKDIR /app
ENV ASPNETCORE_HTTP_PORTS=8080 \
    DOTNET_EnableDiagnostics=0 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
USER $APP_UID
COPY --from=build --chown=$APP_UID:$APP_UID /out ./
ENTRYPOINT ["dotnet", "Sapphirus.DesktopSupportApi.dll"]
