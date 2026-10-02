// Render the canonical vector mark as a native app icon, including its
// transparent margin. AppKit does not mask custom application icon NSImages.
import AppKit

guard CommandLine.arguments.count == 3 else {
    fatalError("Usage: render-app-icon.swift topo-logo.svg output.png")
}
let source = try String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: 1024, pixelsHigh: 1024,
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
)!
let graphics = NSGraphicsContext(bitmapImageRep: bitmap)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = graphics
let context = graphics.cgContext
context.clear(CGRect(x: 0, y: 0, width: 1024, height: 1024))
context.translateBy(x: 0, y: 1024)
context.scaleBy(x: 2, y: -2)

// Continuous corners and 10% outer padding follow the proportions of macOS
// application icons. The outer canvas stays transparent at every resolution.
let tile = CGMutablePath()
tile.move(to: CGPoint(x: 151, y: 50))
tile.addLine(to: CGPoint(x: 361, y: 50))
tile.addCurve(to: CGPoint(x: 462, y: 151), control1: CGPoint(x: 434, y: 50), control2: CGPoint(x: 462, y: 78))
tile.addLine(to: CGPoint(x: 462, y: 361))
tile.addCurve(to: CGPoint(x: 361, y: 462), control1: CGPoint(x: 462, y: 434), control2: CGPoint(x: 434, y: 462))
tile.addLine(to: CGPoint(x: 151, y: 462))
tile.addCurve(to: CGPoint(x: 50, y: 361), control1: CGPoint(x: 78, y: 462), control2: CGPoint(x: 50, y: 434))
tile.addLine(to: CGPoint(x: 50, y: 151))
tile.addCurve(to: CGPoint(x: 151, y: 50), control1: CGPoint(x: 50, y: 78), control2: CGPoint(x: 78, y: 50))
tile.closeSubpath()
context.saveGState()
context.addPath(tile)
context.clip()
let gradient = CGGradient(
    colorsSpace: CGColorSpaceCreateDeviceRGB(),
    colors: [CGColor(red: 0.105, green: 0.109, blue: 0.125, alpha: 1),
             CGColor(red: 0.067, green: 0.071, blue: 0.086, alpha: 1)] as CFArray,
    locations: [0, 1]
)!
context.drawLinearGradient(gradient, start: CGPoint(x: 256, y: 50), end: CGPoint(x: 256, y: 462), options: [])
context.restoreGState()
context.addPath(tile)
context.setStrokeColor(CGColor(gray: 1, alpha: 0.14))
context.setLineWidth(1)
context.strokePath()

// The logo remains sourced from the canonical SVG rather than from the opaque
// preview. Support its explicit M/L/C/Z commands and fail on unsupported edits.
let pathPattern = try NSRegularExpression(pattern: #"<path\s+d="([^"]+)""#)
let tokenPattern = try NSRegularExpression(pattern: #"[A-Za-z]|-?(?:\d*\.)?\d+"#)
let paths = pathPattern.matches(in: source, range: NSRange(source.startIndex..., in: source))
guard !paths.isEmpty else { fatalError("No SVG paths found") }
context.saveGState()
context.translateBy(x: 51.2, y: 51.2)
context.scaleBy(x: 0.8, y: 0.8)
context.setFillColor(CGColor(gray: 1, alpha: 1))
for match in paths {
    let data = String(source[Range(match.range(at: 1), in: source)!])
    let tokens = tokenPattern.matches(in: data, range: NSRange(data.startIndex..., in: data))
        .map { String(data[Range($0.range, in: data)!]) }
    var index = 0
    func point() -> CGPoint {
        guard index + 1 < tokens.count,
              let x = Double(tokens[index]), let y = Double(tokens[index + 1]) else {
            fatalError("Invalid SVG coordinate")
        }
        index += 2
        return CGPoint(x: x, y: y)
    }
    let path = CGMutablePath()
    while index < tokens.count {
        let command = tokens[index]
        index += 1
        switch command {
        case "M": path.move(to: point())
        case "L": path.addLine(to: point())
        case "C":
            let first = point(), second = point(), end = point()
            path.addCurve(to: end, control1: first, control2: second)
        case "Z": path.closeSubpath()
        default: fatalError("Unsupported SVG command: \(command)")
        }
    }
    context.addPath(path)
    context.fillPath()
}
context.restoreGState()
NSGraphicsContext.restoreGraphicsState()
try bitmap.representation(using: .png, properties: [:])!
    .write(to: URL(fileURLWithPath: CommandLine.arguments[2]))
