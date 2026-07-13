# syntax=docker/dockerfile:1.7
FROM mcr.microsoft.com/dotnet/sdk:10.0.100-alpine3.22@sha256:REPLACE_WITH_REVIEWED_BUILD_DIGEST AS build
WORKDIR /src
COPY Sapphirus.DesktopSupportApi.csproj ./
RUN dotnet restore --locked-mode
COPY . ./
RUN dotnet publish --no-restore --configuration Release --output /out \
    /p:UseAppHost=false /p:PublishReadyToRun=false

FROM mcr.microsoft.com/dotnet/aspnet:10.0.0-alpine3.22@sha256:REPLACE_WITH_REVIEWED_RUNTIME_DIGEST AS runtime
WORKDIR /app
ENV ASPNETCORE_HTTP_PORTS=8080 \
    DOTNET_EnableDiagnostics=0 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
USER $APP_UID
COPY --from=build --chown=$APP_UID:$APP_UID /out ./
ENTRYPOINT ["dotnet", "Sapphirus.DesktopSupportApi.dll"]
