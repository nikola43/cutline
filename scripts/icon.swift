import AppKit
let bitmap=NSBitmapImageRep(bitmapDataPlanes:nil,pixelsWide:1024,pixelsHigh:1024,bitsPerSample:8,samplesPerPixel:4,hasAlpha:true,isPlanar:false,colorSpaceName:.deviceRGB,bytesPerRow:0,bitsPerPixel:0)!
NSGraphicsContext.current=NSGraphicsContext(bitmapImageRep:bitmap)
NSColor(calibratedRed:1,green:0.89,blue:0.04,alpha:1).setFill()
NSBezierPath(roundedRect:NSRect(x:20,y:20,width:984,height:984),xRadius:220,yRadius:220).fill()
NSColor(calibratedRed:0.08,green:0.09,blue:0.08,alpha:1).setStroke()
for y in [360,650] {let p=NSBezierPath(ovalIn:NSRect(x:210,y:y,width:200,height:200));p.lineWidth=52;p.stroke()}
for (a,b) in [(NSPoint(x:385,y:410),NSPoint(x:770,y:780)),(NSPoint(x:385,y:700),NSPoint(x:770,y:280))] {let p=NSBezierPath();p.move(to:a);p.line(to:b);p.lineWidth=56;p.lineCapStyle = .round;p.stroke()}
try bitmap.representation(using:.png,properties:[:])!.write(to:URL(fileURLWithPath:"scripts/icon.png"))
